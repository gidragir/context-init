//! MCP tool execution handlers with token budgeting, Git staleness validation, and telemetry (D14, D19, D21, D29).

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use ci_core::analysis::pack::{pack_subsystem, PackStyle, PackedSubsystems};
use ci_core::analysis::resolve_knowledge_dir;
use ci_core::analysis::scan::{render_scan_markdown, scan_project};
use ci_core::analysis::task::{build_task_digest, TaskDigestRequest};
use ci_core::config::{CliOverrides, Config, StorageMode, StoragePaths};
use ci_core::git::{check_card_staleness, GitBackend, SystemGitBackend};
use ci_core::openviking::OkfCard;
use ci_core::storage::{
    compute_project_id, now_rfc3339, safe_join, subsystem_card_path, L0Index, SubsystemInfo,
};
use ci_core::telemetry::{
    record_feedback, FeedbackSignal, TelemetryEvent, TelemetrySignal, TelemetrySink,
};
use ci_llm::enrich::{cli_outcome_message, EnrichOutcome};
use ci_llm::providers::validate_agent_submission;
use ci_llm::{AgentWorkPackage, RoleRouter};

use crate::pipeline::{self, PipelineOptions};

use crate::pagination::{count_tokens, paginate_content};
use crate::tools::*;

/// `pack all` is refused when the packed project exceeds this many response pages (D19).
pub const PACK_ALL_MAX_PAGES: usize = 5;

/// Context bundle describing a dynamically resolved project root and its knowledge store (D21).
#[derive(Debug, Clone)]
pub struct ResolvedProject {
    /// Canonical filesystem root of the project.
    pub root: PathBuf,
    /// Loaded configuration layers for this project.
    pub config: Config,
    /// Normalized project identifier.
    pub project_id: String,
    /// Resolved storage paths hierarchy.
    pub storage_paths: StoragePaths,
    /// Active knowledge base directory, chosen by `config.storage`.
    pub knowledge_dir: PathBuf,
    /// Configured storage mode (`config.storage`).
    pub storage: StorageMode,
}

impl ResolvedProject {
    /// Loads the project context for `root`, resolving configuration and storage paths.
    pub fn load(root: PathBuf) -> Result<Self> {
        let canonical_root = fs::canonicalize(&root).unwrap_or_else(|_| root.clone());
        let config =
            Config::load(Some(&canonical_root), &CliOverrides::default()).unwrap_or_default();
        Ok(Self::with_config(canonical_root, config))
    }

    /// Strict variant for the CLI: configuration errors are returned, CLI overrides and an
    /// optional extra config file (`--config`, applied above project/global files) are honored.
    pub fn load_with(
        root: PathBuf,
        cli: &CliOverrides,
        extra_config: Option<&Path>,
    ) -> Result<Self> {
        let canonical_root = fs::canonicalize(&root)
            .with_context(|| format!("Project path {:?} does not exist", root))?;
        let mut config = Config::load(Some(&canonical_root), cli)?;
        if let Some(extra) = extra_config {
            let text = fs::read_to_string(extra)
                .with_context(|| format!("Failed to read config file {:?}", extra))?;
            config.apply_toml_str(&text)?;
            config.apply_cli_overrides(cli)?;
        }
        Ok(Self::with_config(canonical_root, config))
    }

    fn with_config(canonical_root: PathBuf, config: Config) -> Self {
        let git_backend = SystemGitBackend::new();
        let remote = git_backend.remote_url(&canonical_root).ok().flatten();
        let project_id = compute_project_id(&canonical_root, remote.as_deref());

        let storage_paths = StoragePaths::resolve(Some(&canonical_root));
        let storage = config.storage;
        let central_kb = storage_paths.project_knowledge_dir(&project_id);
        let knowledge_dir = resolve_knowledge_dir(&canonical_root, storage, &central_kb);

        Self {
            root: canonical_root,
            config,
            project_id,
            storage_paths,
            knowledge_dir,
            storage,
        }
    }

    /// Knowledge dir the project would use under `mode`.
    pub fn knowledge_dir_for(&self, mode: StorageMode) -> PathBuf {
        resolve_knowledge_dir(
            &self.root,
            mode,
            &self.storage_paths.project_knowledge_dir(&self.project_id),
        )
    }

    /// Loads the `L0Index` from the active knowledge base.
    pub fn load_l0_index(&self) -> Result<L0Index> {
        let index_path = self.knowledge_dir.join("L0_index.json");
        if !index_path.is_file() {
            let mut msg = format!(
                "Knowledge base not found at {:?}. Run context_init first.",
                index_path
            );
            if let Some(hint) = pipeline::storage_mismatch_hint(self) {
                msg.push('\n');
                msg.push_str(&hint);
            }
            bail!(msg);
        }
        L0Index::load_from_file(&index_path)
    }

    /// The subsystem's L1 card inside the knowledge dir: `l1_path`, else `subsystems/<id>.md`.
    /// Index paths that would leave the knowledge dir are ignored.
    pub fn card_file(&self, subsystem: &SubsystemInfo) -> Option<PathBuf> {
        let norm_l1 = subsystem.l1_path.replace('\\', "/");
        safe_join(&self.knowledge_dir, &norm_l1)
            .filter(|p| p.is_file())
            .or_else(|| {
                subsystem_card_path(&self.knowledge_dir, &subsystem.id)
                    .ok()
                    .filter(|p| p.is_file())
            })
    }

    /// Checks if a subsystem card is stale relative to Git.
    pub fn check_subsystem_staleness(&self, subsystem: &SubsystemInfo) -> (bool, Option<String>) {
        let Some(path) = self.card_file(subsystem) else {
            return (false, None);
        };

        let Ok(content) = fs::read_to_string(&path) else {
            return (false, None);
        };

        if let Ok(card) = OkfCard::parse(&content) {
            let backend = SystemGitBackend::new();
            if let Ok(report) = check_card_staleness(&backend, &self.root, &card) {
                if report.stale {
                    return (true, Some(report.diff_stat));
                }
            }
        }

        (false, None)
    }
}

/// Reply of a handler that may hand an agent work package to the server.
#[derive(Debug, Clone)]
pub struct HandlerReply {
    /// Paginated text returned to the MCP client.
    pub text: String,
    /// Work package the server must register for a later `context_submit`.
    pub pending_package: Option<AgentWorkPackage>,
}

/// Core handler implementations for MCP tools.
pub struct ContextHandler;

impl ContextHandler {
    /// Handler for `context_check`.
    pub fn check_drift(project: &ResolvedProject, cursor: Option<&str>) -> Result<String> {
        let pipeline::FullDriftReport {
            has_drift,
            missing_in_fs,
            untracked_in_knowledge,
            up_to_date_count,
            git_stale,
            ..
        } = pipeline::run_check(project)?;

        let message = if !has_drift {
            format!("OK: {} subsystems verified up-to-date", up_to_date_count)
        } else {
            format!(
                "DRIFT: up_to_date={}, missing=[{}], untracked=[{}], git_stale={}",
                up_to_date_count,
                missing_in_fs.join(", "),
                untracked_in_knowledge.join(", "),
                git_stale
            )
        };

        let result = McpCheckResult {
            has_drift,
            missing_in_fs,
            untracked_in_knowledge,
            up_to_date_count,
            git_stale,
            message,
        };

        let json_text = serde_json::to_string_pretty(&result)?;
        let paginated = paginate_content(
            &json_text,
            cursor,
            project.config.max_response_tokens,
            git_stale,
        );
        Ok(paginated.rendered)
    }

    /// Handler for `context_get_subsystems`.
    pub fn get_subsystems(project: &ResolvedProject, cursor: Option<&str>) -> Result<String> {
        let index = project.load_l0_index()?;
        let mut any_stale = false;
        for sub in &index.subsystems {
            let (stale, _) = project.check_subsystem_staleness(sub);
            if stale {
                any_stale = true;
                break;
            }
        }

        // Sizes are not stored in the base: count them live from the current sources.
        let listing: Vec<serde_json::Value> = index
            .subsystems
            .iter()
            .map(|sub| {
                let mut v = serde_json::to_value(sub)?;
                v["tokens"] =
                    ci_core::analysis::subsystems::subsystem_tokens(&project.root, sub).into();
                Ok(v)
            })
            .collect::<Result<_>>()?;
        let json_text = serde_json::to_string_pretty(&listing)?;
        let paginated = paginate_content(
            &json_text,
            cursor,
            project.config.max_response_tokens,
            any_stale,
        );
        Ok(paginated.rendered)
    }

    /// Handler for `context_get_subsystem`.
    pub fn get_subsystem(
        project: &ResolvedProject,
        subsystem_id: &str,
        cursor: Option<&str>,
    ) -> Result<String> {
        let index = project.load_l0_index()?;
        let clean_id = subsystem_id
            .trim()
            .trim_start_matches("subsystems/")
            .trim_start_matches("subsystems\\")
            .trim_end_matches(".md");

        let matched = index.subsystems.iter().find(|s| {
            s.id.eq_ignore_ascii_case(subsystem_id)
                || s.name.eq_ignore_ascii_case(subsystem_id)
                || s.id.eq_ignore_ascii_case(clean_id)
                || s.name.eq_ignore_ascii_case(clean_id)
        });

        let Some(sub) = matched else {
            // Metadata objects have no L1 card: describe them lazily (L2, D8).
            if let Some(card) = ci_core::analysis::lang::mdo::find_object_card(
                &project.root,
                &project.config.scope,
                clean_id,
            )? {
                return Ok(paginate_content(
                    &card,
                    cursor,
                    project.config.max_response_tokens,
                    false,
                )
                .rendered);
            }
            let available: Vec<&str> = index.subsystems.iter().map(|s| s.id.as_str()).collect();
            bail!(
                "Subsystem '{}' not found in L0 index. Available subsystems: {}",
                subsystem_id,
                available.join(", ")
            );
        };

        let (mut stale, mut diff_stat) = project.check_subsystem_staleness(sub);
        if stale && project.config.auto_refresh {
            // Lazy refresh (D2): re-stamp the stale card; a failure leaves it reported as stale.
            if pipeline::refresh_stale_card(project, sub).is_ok() {
                (stale, diff_stat) = project.check_subsystem_staleness(sub);
            }
        }

        // Try reading markdown card file (provenance frontmatter is for tooling, not for agents)
        let mut card_content = if let Some(card) = project.card_file(sub) {
            let text = fs::read_to_string(&card)?;
            match OkfCard::parse(&text) {
                Ok(parsed) => parsed.body,
                Err(_) => text,
            }
        } else {
            // Synthesize compact L1 card from L0 metadata
            let tokens = ci_core::analysis::subsystems::subsystem_tokens(&project.root, sub);
            let mut card = format!(
                "# Subsystem: {} (L1)\n\n> **ID:** `{}`\n> **Tags:** {} | **Tokens:** ~{} BPE\n\n## Summary\n{}\n\n",
                sub.name,
                sub.id,
                sub.tags.join(", "),
                tokens,
                sub.summary
            );
            if !sub.entrypoints.is_empty() {
                card.push_str("## Entrypoints\n");
                for ep in &sub.entrypoints {
                    card.push_str(&format!("- `{}`\n", ep));
                }
                card.push('\n');
            }
            if !sub.key_files.is_empty() {
                card.push_str("## Key Files\n");
                for kf in &sub.key_files {
                    card.push_str(&format!("- `{}`\n", kf));
                }
                card.push('\n');
            }
            card
        };

        if stale {
            if let Some(diff) = diff_stat {
                card_content.push_str(&format!(
                    "\n\n## Git Staleness (diff --stat)\n```text\n{}\n```\n",
                    diff.trim()
                ));
            }
        }

        let paginated = paginate_content(
            &card_content,
            cursor,
            project.config.max_response_tokens,
            stale,
        );
        Ok(paginated.rendered)
    }

    /// Handler for `context_pack`.
    pub fn pack(
        project: &ResolvedProject,
        subsystem_id: &str,
        compress: bool,
        style: &str,
        cursor: Option<&str>,
    ) -> Result<String> {
        let index = project.load_l0_index()?;
        let PackedSubsystems {
            content: packed_content,
            subsystems: matched_subs,
            ..
        } = pack_subsystem(
            &project.root,
            &project.knowledge_dir,
            &index,
            subsystem_id,
            compress,
            PackStyle::parse(style),
        )?;

        let budget = project.config.max_response_tokens;
        if subsystem_id.eq_ignore_ascii_case("all")
            && count_tokens(&packed_content) > PACK_ALL_MAX_PAGES.saturating_mul(budget)
        {
            let ids: Vec<&str> = index.subsystems.iter().map(|s| s.id.as_str()).collect();
            let refusal = format!(
                "Refusing pack all: the packed project exceeds {PACK_ALL_MAX_PAGES} pages of {budget} tokens (D19). \
                 Pack per subsystem instead with context_pack(subsystem_id=<id>). Subsystems: {}",
                ids.join(", ")
            );
            return Ok(paginate_content(&refusal, cursor, budget, false).rendered);
        }

        let any_stale = matched_subs
            .iter()
            .any(|s| project.check_subsystem_staleness(s).0);

        let paginated = paginate_content(
            &packed_content,
            cursor,
            project.config.max_response_tokens,
            any_stale,
        );
        Ok(paginated.rendered)
    }

    /// Persists a validated agent submission into the knowledge base.
    ///
    /// Every subsystem id is validated before anything is written, so a hostile id can
    /// neither escape `knowledge_dir` nor leave a half-applied update behind. File lists
    /// are never taken from submissions: they come from the detector only.
    fn persist_submission(project: &ResolvedProject, payload: &serde_json::Value) -> Result<()> {
        let kd = &project.knowledge_dir;
        let index_path = kd.join("L0_index.json");
        let mut index = if index_path.is_file() {
            project.load_l0_index()?
        } else {
            L0Index::default()
        };
        let str_field = |v: &serde_json::Value, key: &str| {
            v.get(key)
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned)
        };
        let tags_field = |v: &serde_json::Value| {
            v.get("tags")
                .and_then(|t| serde_json::from_value::<Vec<String>>(t.clone()).ok())
        };

        if let Some(items) = payload
            .get("subsystems")
            .and_then(serde_json::Value::as_array)
        {
            let mut cards = Vec::with_capacity(items.len());
            for item in items {
                let id = str_field(item, "id").unwrap_or_default();
                cards.push((subsystem_card_path(kd, &id)?, id, item));
            }
            for (card_path, id, item) in cards {
                let name = str_field(item, "name").unwrap_or_else(|| id.clone());
                let summary = str_field(item, "summary").unwrap_or_default();
                if let Some(sub) = index.subsystems.iter_mut().find(|s| s.id == id) {
                    sub.name = name.clone();
                    sub.summary = summary.clone();
                    if let Some(tags) = tags_field(item) {
                        sub.tags = tags;
                    }
                } else {
                    index.subsystems.push(SubsystemInfo {
                        id: id.clone(),
                        name: name.clone(),
                        summary: summary.clone(),
                        l1_path: format!("subsystems/{id}.md"),
                        tags: tags_field(item).unwrap_or_default(),
                        entrypoints: Vec::new(),
                        key_files: Vec::new(),
                        files: Vec::new(),
                        key_symbols: Vec::new(),
                        test_command: None,
                        tokens: 0,
                        parent: None,
                    });
                }
                if !card_path.exists() {
                    fs::create_dir_all(kd.join("subsystems"))?;
                    fs::write(
                        &card_path,
                        format!("# Subsystem: {name} (L1)\n\n{summary}\n"),
                    )
                    .with_context(|| format!("Failed to write card {card_path:?}"))?;
                }
            }
        } else if let Some(id) = str_field(payload, "id") {
            let card_path = subsystem_card_path(kd, &id)?;
            if let Some(body) = str_field(payload, "body") {
                fs::create_dir_all(kd.join("subsystems"))
                    .with_context(|| format!("Failed to create {:?}", kd.join("subsystems")))?;
                fs::write(&card_path, body)
                    .with_context(|| format!("Failed to write card {card_path:?}"))?;
            }
            if let Some(sub) = index.subsystems.iter_mut().find(|s| s.id == id) {
                if let Some(summary) = str_field(payload, "summary") {
                    sub.summary = summary;
                }
                if let Some(tags) = tags_field(payload) {
                    sub.tags = tags;
                }
            }
        } else {
            return Ok(());
        }

        index.updated_at = now_rfc3339();
        fs::create_dir_all(kd)?;
        index.save_to_file(&index_path)
    }

    /// Handler for `context_submit` (D14, D29, R4).
    pub fn submit(
        project: &ResolvedProject,
        package_id: &str,
        work_package: Option<&AgentWorkPackage>,
        submission: &serde_json::Value,
    ) -> Result<String> {
        let Some(pkg_ref) = work_package else {
            let result = SubmitResult {
                valid: false,
                package_id: package_id.to_string(),
                role: String::new(),
                errors: vec![format!(
                    "Unknown package_id '{package_id}': request a work package via context_init/context_update first"
                )],
                message: "Agent submission rejected: unknown work package.".to_string(),
            };
            let json_text = serde_json::to_string_pretty(&result)?;
            return Ok(paginate_content(
                &json_text,
                None,
                project.config.max_response_tokens,
                false,
            )
            .rendered);
        };

        match validate_agent_submission(pkg_ref, submission) {
            Ok(validated) => {
                if let Err(e) = Self::persist_submission(project, &validated.payload) {
                    let result = SubmitResult {
                        valid: false,
                        package_id: validated.package_id,
                        role: validated.role.to_string(),
                        errors: e.chain().map(|c| c.to_string()).collect(),
                        message: "Agent submission rejected: it could not be persisted."
                            .to_string(),
                    };
                    let json_text = serde_json::to_string_pretty(&result)?;
                    return Ok(paginate_content(
                        &json_text,
                        None,
                        project.config.max_response_tokens,
                        false,
                    )
                    .rendered);
                }

                let result = SubmitResult {
                    valid: true,
                    package_id: validated.package_id,
                    role: validated.role.to_string(),
                    errors: Vec::new(),
                    message: "Agent submission successfully validated and accepted.".to_string(),
                };
                let json_text = serde_json::to_string_pretty(&result)?;
                let paginated =
                    paginate_content(&json_text, None, project.config.max_response_tokens, false);
                Ok(paginated.rendered)
            }
            Err(e) => {
                let mut errors = Vec::new();
                for cause in e.chain() {
                    let s = cause.to_string();
                    if !errors.contains(&s) {
                        errors.push(s);
                    }
                }
                let result = SubmitResult {
                    valid: false,
                    package_id: package_id.to_string(),
                    role: pkg_ref.role.to_string(),
                    errors,
                    message: "Agent submission rejected: JSON schema validation failed."
                        .to_string(),
                };
                let json_text = serde_json::to_string_pretty(&result)?;
                let paginated =
                    paginate_content(&json_text, None, project.config.max_response_tokens, false);
                Ok(paginated.rendered)
            }
        }
    }

    /// Handler for `context_feedback` (D29, R4).
    pub fn feedback(
        project: &ResolvedProject,
        tool: &str,
        verdict: &str,
        note: Option<&str>,
        telemetry_sink: Option<&dyn TelemetrySink>,
    ) -> Result<String> {
        let signal = FeedbackSignal {
            tool: tool.to_string(),
            verdict: verdict.to_string(),
            note: note.map(|n| n.to_string()),
        };

        let recorded = match telemetry_sink {
            Some(sink) => {
                let event =
                    TelemetryEvent::new(&project.project_id, TelemetrySignal::Feedback(signal));
                sink.is_enabled() && sink.record(&event).is_ok()
            }
            None => record_feedback(
                &project.config,
                &project.storage_paths,
                &project.project_id,
                signal,
            )
            .unwrap_or(false),
        };

        let result = FeedbackResult {
            status: "ok".to_string(),
            tool: tool.to_string(),
            verdict: verdict.to_string(),
            recorded,
        };

        let json_text = serde_json::to_string_pretty(&result)?;
        let paginated =
            paginate_content(&json_text, None, project.config.max_response_tokens, false);
        Ok(paginated.rendered)
    }

    /// Handler for `context_scan` (D9).
    pub fn scan(project: &ResolvedProject, cursor: Option<&str>) -> Result<String> {
        let report = scan_project(&project.root, &project.config.scope)?;
        let scan_out = render_scan_markdown(&project.project_id, &project.root, &report);
        let paginated =
            paginate_content(&scan_out, cursor, project.config.max_response_tokens, false);
        Ok(paginated.rendered)
    }

    /// Handler for `context_init` (pipeline in [`crate::pipeline::run_init`]).
    pub fn init(
        project: &ResolvedProject,
        params: &McpInitParams,
        router: &RoleRouter,
    ) -> Result<HandlerReply> {
        let opts = PipelineOptions {
            deep: params.deep.unwrap_or(false),
            force: params.force.unwrap_or(false),
            dry_run: false,
            with_agents_md: params.with_agents_md.unwrap_or(true),
            model_override: params.model.as_deref(),
        };
        let hint = pipeline::storage_mismatch_hint(project);
        let out = pipeline::run_init(project, router, &opts)?;
        let mut text = format!(
            "Knowledge base initialized at {} ({} subsystems).",
            out.knowledge_dir.display(),
            out.subsystems.len()
        );
        if let Some(hint) = hint {
            text.push_str(&format!("\n{hint}"));
        }
        Ok(Self::finish_reply(project, text, out.enrich))
    }

    /// Handler for `context_update` (pipeline in [`crate::pipeline::run_update`]).
    pub fn update(
        project: &ResolvedProject,
        params: &McpUpdateParams,
        router: &RoleRouter,
    ) -> Result<HandlerReply> {
        let opts = PipelineOptions {
            deep: params.deep.unwrap_or(false),
            with_agents_md: project.config.touch_agents_md,
            model_override: params.model.as_deref(),
            ..PipelineOptions::default()
        };
        let hint = pipeline::storage_mismatch_hint(project);
        let out = pipeline::run_update(project, router, &opts)?;
        let mut text = format!(
            "Knowledge base updated at {}: {} added, {} refreshed.",
            out.knowledge_dir.display(),
            out.added,
            out.updated
        );
        if let Some(hint) = hint {
            text.push_str(&format!("\n{hint}"));
        }
        Ok(Self::finish_reply(project, text, out.enrich))
    }

    /// Appends the enrichment result (and an agent work package, if any) and paginates.
    fn finish_reply(
        project: &ResolvedProject,
        mut text: String,
        outcome: EnrichOutcome,
    ) -> HandlerReply {
        let pending_package = match outcome {
            EnrichOutcome::PendingAgent(pkg) => {
                text.push_str(&format!(
                    "\n\nAgent work package — improve the subsystem summaries and answer with \
                     context_submit(package_id, submission).\npackage_id: {}\ninstruction: {}\njson_schema: {}\nfacts: {}",
                    pkg.package_id, pkg.instruction, pkg.json_schema, pkg.facts
                ));
                Some(pkg)
            }
            other => {
                if let Some(msg) = cli_outcome_message(&other) {
                    text.push_str(&format!("\n{msg}"));
                }
                None
            }
        };
        HandlerReply {
            text: paginate_content(&text, None, project.config.max_response_tokens, false).rendered,
            pending_package,
        }
    }

    /// Handler for `context_task` (D4, D5): L1 digest, or the L2 hunk of `target`.
    pub fn task(
        project: &ResolvedProject,
        paths: Option<&[String]>,
        base_ref: Option<&str>,
        target: Option<&str>,
        cursor: Option<&str>,
    ) -> Result<String> {
        let digest = build_task_digest(
            &project.root,
            &SystemGitBackend::new(),
            project.load_l0_index().ok().as_ref(),
            &TaskDigestRequest {
                paths: paths.unwrap_or(&[]),
                base_ref,
                target,
                max_tokens: project.config.max_response_tokens,
            },
        )?;
        let paginated =
            paginate_content(&digest, cursor, project.config.max_response_tokens, false);
        Ok(paginated.rendered)
    }

    /// Handler for `context_search` (D18).
    pub fn search(project: &ResolvedProject, query: &str, cursor: Option<&str>) -> Result<String> {
        let index = project.load_l0_index()?;
        let query_lower = query.to_ascii_lowercase();

        let mut matches = Vec::new();
        for sub in &index.subsystems {
            if sub.id.to_ascii_lowercase().contains(&query_lower)
                || sub.name.to_ascii_lowercase().contains(&query_lower)
                || sub.summary.to_ascii_lowercase().contains(&query_lower)
                || sub
                    .tags
                    .iter()
                    .any(|t| t.to_ascii_lowercase().contains(&query_lower))
                || sub
                    .key_symbols
                    .iter()
                    .any(|s| s.to_ascii_lowercase().contains(&query_lower))
                || sub
                    .files
                    .iter()
                    .any(|f| f.path.to_ascii_lowercase().contains(&query_lower))
                || sub
                    .key_files
                    .iter()
                    .any(|kf| kf.to_ascii_lowercase().contains(&query_lower))
                || sub
                    .entrypoints
                    .iter()
                    .any(|ep| ep.to_ascii_lowercase().contains(&query_lower))
            {
                matches.push(sub);
            }
        }

        let mut search_out = format!(
            "# Search Results for '{}'\n\nFound {} matching subsystems:\n\n",
            query,
            matches.len()
        );

        for m in matches {
            search_out.push_str(&format!(
                "### `{}` ({})\n{}\n> **Tags:** {}\n\n",
                m.id,
                m.name,
                m.summary,
                m.tags.join(", ")
            ));
        }

        let paginated = paginate_content(
            &search_out,
            cursor,
            project.config.max_response_tokens,
            false,
        );
        Ok(paginated.rendered)
    }
}
