//! MCP tool execution handlers with token budgeting, Git staleness validation, and telemetry (D14, D19, D21, D29).

use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use ci_core::analysis::generator::{self, GenerateOptions};
use ci_core::analysis::pack::{pack_files, PackStyle};
use ci_core::analysis::subsystems::build_deterministic;
use ci_core::analysis::{detector, resolve_knowledge_dir, KnowledgeLocation, ProjectAnalysis};
use ci_core::config::{CliOverrides, Config, StoragePaths};
use ci_core::git::{check_card_staleness, GitBackend, SystemGitBackend};
use ci_core::openviking::OkfCard;
use ci_core::storage::{
    compute_project_id, now_rfc3339, subsystem_card_path, L0Index, SubsystemInfo,
};
use ci_core::telemetry::{
    FeedbackSignal, JsonlTelemetrySink, TelemetryEvent, TelemetrySignal, TelemetrySink,
};
use ci_llm::enrich::{cli_outcome_message, enrich_subsystems, EnrichOutcome};
use ci_llm::providers::validate_agent_submission;
use ci_llm::{AgentWorkPackage, PromptTemplateRegistry, RoleName, RoleRouter};

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
    /// Active knowledge base directory (central storage or in-repo fallback).
    pub knowledge_dir: PathBuf,
    /// Indicates whether `knowledge_dir` points to central storage (`~/.local/share/context-init/projects/...`).
    pub is_central: bool,
}

impl ResolvedProject {
    /// Loads the project context for `root`, resolving configuration and storage paths.
    pub fn load(root: PathBuf) -> Result<Self> {
        let canonical_root = fs::canonicalize(&root).unwrap_or_else(|_| root.clone());
        let config =
            Config::load(Some(&canonical_root), &CliOverrides::default()).unwrap_or_default();

        let git_backend = SystemGitBackend::new();
        let remote = git_backend.remote_url(&canonical_root).ok().flatten();
        let project_id = compute_project_id(&canonical_root, remote.as_deref());

        let storage_paths = StoragePaths::resolve(Some(&canonical_root));
        let central_kb = storage_paths.project_knowledge_dir(&project_id);

        let (knowledge_dir, location) = resolve_knowledge_dir(&canonical_root, &central_kb);
        let is_central = location == KnowledgeLocation::Central;

        Ok(Self {
            root: canonical_root,
            config,
            project_id,
            storage_paths,
            knowledge_dir,
            is_central,
        })
    }

    /// Loads the `L0Index` from the active knowledge base.
    pub fn load_l0_index(&self) -> Result<L0Index> {
        let index_path = self.knowledge_dir.join("L0_index.json");
        if !index_path.is_file() {
            bail!(
                "Knowledge base not found at {:?}. Run context_init first.",
                index_path
            );
        }
        L0Index::load_from_file(&index_path)
    }

    /// Checks if a subsystem card is stale relative to Git.
    pub fn check_subsystem_staleness(&self, subsystem: &SubsystemInfo) -> (bool, Option<String>) {
        let norm_l1 = subsystem.l1_path.replace('\\', "/");
        let l1_file = self.knowledge_dir.join(&norm_l1);
        let alt_file = self
            .knowledge_dir
            .join("subsystems")
            .join(format!("{}.md", subsystem.id));

        let card_path = if l1_file.is_file() {
            Some(l1_file)
        } else if alt_file.is_file() {
            Some(alt_file)
        } else {
            None
        };

        let Some(path) = card_path else {
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
        let (has_drift, missing_in_fs, untracked_in_knowledge, up_to_date_count, git_stale) = match (
            project.load_l0_index(),
            detector::analyze_project(&project.root, false, false),
        ) {
            (Ok(index), Ok(analysis)) => {
                let report = generator::audit(&analysis, &project.knowledge_dir)?;
                let stale_count = index
                    .subsystems
                    .iter()
                    .filter(|s| project.check_subsystem_staleness(s).0)
                    .count();
                let drift = !report.missing_in_fs.is_empty()
                    || !report.untracked_in_knowledge.is_empty()
                    || stale_count > 0;
                (
                    drift,
                    report.missing_in_fs,
                    report.untracked_in_knowledge,
                    report.up_to_date_count.saturating_sub(stale_count),
                    stale_count > 0,
                )
            }
            _ => (true, Vec::new(), Vec::new(), 0, false),
        };

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

        let json_text = serde_json::to_string_pretty(&index.subsystems)?;
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
            let available: Vec<&str> = index.subsystems.iter().map(|s| s.id.as_str()).collect();
            bail!(
                "Subsystem '{}' not found in L0 index. Available subsystems: {}",
                subsystem_id,
                available.join(", ")
            );
        };

        let (stale, diff_stat) = project.check_subsystem_staleness(sub);

        // Try reading markdown card file
        let l1_file = project.knowledge_dir.join(&sub.l1_path);
        let alt_file = project
            .knowledge_dir
            .join("subsystems")
            .join(format!("{}.md", sub.id));

        let mut card_content = if l1_file.is_file() {
            fs::read_to_string(&l1_file)?
        } else if alt_file.is_file() {
            fs::read_to_string(&alt_file)?
        } else {
            // Synthesize compact L1 card from L0 metadata
            let mut card = format!(
                "# Subsystem: {} (L1)\n\n> **ID:** `{}`\n> **Tags:** {} | **Tokens:** ~{} BPE\n\n## Summary\n{}\n\n",
                sub.name,
                sub.id,
                sub.tags.join(", "),
                sub.tokens,
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
                if !card_content.contains("Warning: Stale Knowledge Card") {
                    card_content.push_str(&format!(
                        "\n\n> [!WARNING] **Stale Knowledge Card**\n> Git diff stat since card generation:\n```diff\n{}\n```\n",
                        diff.trim()
                    ));
                }
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

        let matched_subs: Vec<&SubsystemInfo> = if subsystem_id.eq_ignore_ascii_case("all") {
            index.subsystems.iter().collect()
        } else {
            let clean = subsystem_id
                .trim()
                .trim_start_matches("subsystems/")
                .trim_end_matches(".md");
            index
                .subsystems
                .iter()
                .filter(|s| {
                    s.id.eq_ignore_ascii_case(subsystem_id)
                        || s.name.eq_ignore_ascii_case(subsystem_id)
                        || s.id.eq_ignore_ascii_case(clean)
                        || s.name.eq_ignore_ascii_case(clean)
                })
                .collect()
        };

        if matched_subs.is_empty() {
            let available: Vec<&str> = index.subsystems.iter().map(|s| s.id.as_str()).collect();
            bail!(
                "Subsystem '{}' not found in L0 index. Available subsystems: {}",
                subsystem_id,
                available.join(", ")
            );
        }

        let mut files_to_pack: Vec<String> = Vec::new();
        for sub in &matched_subs {
            for f in &sub.files {
                let full = project.root.join(&f.path);
                if full.exists() && !files_to_pack.contains(&f.path) {
                    files_to_pack.push(f.path.clone());
                }
            }
            for kf in &sub.key_files {
                let full = project.root.join(kf);
                if full.exists() && !files_to_pack.contains(kf) {
                    files_to_pack.push(kf.clone());
                }
            }
        }

        if files_to_pack.is_empty() {
            bail!(
                "No existing source files found on disk for subsystem '{}'",
                subsystem_id
            );
        }

        let packed_content = pack_files(
            &project.root,
            &matched_subs,
            &files_to_pack,
            compress,
            PackStyle::parse(style),
        );

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

        let event = TelemetryEvent::new(&project.project_id, TelemetrySignal::Feedback(signal));

        let recorded = if let Some(sink) = telemetry_sink {
            sink.record(&event).is_ok()
        } else {
            let sink = JsonlTelemetrySink::new(
                &project.storage_paths.data_dir,
                project.config.telemetry.enabled,
            );
            sink.record(&event).is_ok()
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
        let mut extensions_map: HashMap<String, (usize, usize)> = HashMap::new();
        let mut total_files = 0;
        let mut total_lines = 0;

        let mut walker = vec![project.root.clone()];
        while let Some(dir) = walker.pop() {
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with('.') || name == "target" || name == "node_modules" {
                        continue;
                    }
                    if path.is_dir() {
                        walker.push(path);
                    } else if path.is_file() {
                        total_files += 1;
                        let ext = path
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("none")
                            .to_ascii_lowercase();
                        let lines = fs::read_to_string(&path)
                            .map(|c| c.lines().count())
                            .unwrap_or(0);
                        total_lines += lines;
                        let entry = extensions_map.entry(ext).or_insert((0, 0));
                        entry.0 += 1;
                        entry.1 += lines;
                    }
                }
            }
        }

        let mut scan_out = format!(
            "# Project Scan: {}\n\n- **Root:** `{}`\n- **Total Files:** {}\n- **Total Lines:** {}\n\n## Extensions Breakdown\n",
            project.project_id,
            project.root.display(),
            total_files,
            total_lines
        );

        let mut sorted_exts: Vec<_> = extensions_map.into_iter().collect();
        sorted_exts.sort_by_key(|b| std::cmp::Reverse(b.1 .1));
        for (ext, (files, lines)) in sorted_exts {
            scan_out.push_str(&format!("- `.{}`: {} files, {} lines\n", ext, files, lines));
        }

        let paginated =
            paginate_content(&scan_out, cursor, project.config.max_response_tokens, false);
        Ok(paginated.rendered)
    }

    /// Handler for `context_init`: detect → optional LLM enrichment → generate into
    /// central storage (D1, D16, D23).
    pub fn init(
        project: &ResolvedProject,
        params: &McpInitParams,
        router: &RoleRouter,
    ) -> Result<HandlerReply> {
        let analysis = detector::analyze_project(
            &project.root,
            params.deep.unwrap_or(false),
            project.config.integrations.resolve_rtk_active(),
        )?;
        let mut subsystems = build_deterministic(&analysis);
        let outcome = Self::enrich(
            project,
            router,
            RoleName::Init,
            &analysis,
            &mut subsystems,
            params.model.as_deref(),
        );

        let target = project
            .storage_paths
            .project_knowledge_dir(&project.project_id);
        let opts = GenerateOptions {
            force: params.force.unwrap_or(false),
            dry_run: false,
            with_agents_md: params.with_agents_md.unwrap_or(true),
        };
        generator::generate(&analysis, &subsystems, &target, &opts)?;

        let mut text = format!(
            "Knowledge base initialized at {} ({} subsystems).",
            target.display(),
            subsystems.len()
        );
        if !project.is_central {
            text.push_str(&format!(
                "\nA legacy in-repo knowledge base exists at {}; central storage is now authoritative. \
                 Run `context-init migrate --remove-source` to clean it up.",
                project.knowledge_dir.display()
            ));
        }
        Ok(Self::finish_reply(project, text, outcome))
    }

    /// Handler for `context_update`: re-detect and merge into the active knowledge base.
    pub fn update(
        project: &ResolvedProject,
        params: &McpUpdateParams,
        router: &RoleRouter,
    ) -> Result<HandlerReply> {
        let analysis = detector::analyze_project(
            &project.root,
            params.deep.unwrap_or(false),
            project.config.integrations.resolve_rtk_active(),
        )?;
        let mut subsystems = build_deterministic(&analysis);
        let outcome = Self::enrich(
            project,
            router,
            RoleName::Update,
            &analysis,
            &mut subsystems,
            params.model.as_deref(),
        );
        let (added, updated) = generator::update(
            &analysis,
            &subsystems,
            &project.knowledge_dir,
            project.config.touch_agents_md,
        )?;
        let text = format!(
            "Knowledge base updated at {}: {added} added, {updated} refreshed.",
            project.knowledge_dir.display()
        );
        Ok(Self::finish_reply(project, text, outcome))
    }

    fn enrich(
        project: &ResolvedProject,
        router: &RoleRouter,
        role: RoleName,
        analysis: &ProjectAnalysis,
        subsystems: &mut [SubsystemInfo],
        model_override: Option<&str>,
    ) -> EnrichOutcome {
        let prompts = PromptTemplateRegistry::new(project.config.language.clone());
        enrich_subsystems(router, &prompts, role, analysis, subsystems, model_override)
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

    /// Handler for `context_task` (D4, D5).
    pub fn task(
        project: &ResolvedProject,
        paths: Option<&[String]>,
        base_ref: Option<&str>,
        cursor: Option<&str>,
    ) -> Result<String> {
        let backend = SystemGitBackend::new();
        let current_head = backend.head_commit(&project.root).unwrap_or_default();
        let target_base = base_ref.unwrap_or("HEAD~1");

        let diff_names = backend
            .diff_name_only(&project.root, target_base, &current_head)
            .unwrap_or_default();

        let l0_index = project.load_l0_index().ok();

        let mut task_out = format!(
            "# Task Context Digest\n\n- **Project:** `{}`\n- **Base Ref:** `{}`\n- **HEAD:** `{}`\n- **Modified Files:** {}\n\n",
            project.project_id,
            target_base,
            current_head,
            diff_names.len()
        );

        if !diff_names.is_empty() {
            task_out.push_str("## Modified Files (Git Diff)\n");
            for f in &diff_names {
                let sub_tag = l0_index.as_ref().and_then(|idx| {
                    idx.subsystems.iter().find(|s| {
                        s.files.iter().any(|sf| sf.path == *f)
                            || s.key_files.iter().any(|kf| kf == f)
                    })
                });
                if let Some(sub) = sub_tag {
                    task_out.push_str(&format!("- `{}` (subsystem: `{}`)\n", f, sub.id));
                } else {
                    task_out.push_str(&format!("- `{}`\n", f));
                }
            }
            task_out.push('\n');
        }

        if let Some(user_paths) = paths {
            if !user_paths.is_empty() {
                task_out.push_str("## Task-Specified Paths\n");
                for p in user_paths {
                    let sub_tag = l0_index.as_ref().and_then(|idx| {
                        idx.subsystems.iter().find(|s| {
                            s.files.iter().any(|sf| sf.path == *p)
                                || s.key_files.iter().any(|kf| kf == p)
                        })
                    });
                    if let Some(sub) = sub_tag {
                        task_out.push_str(&format!("- `{}` (subsystem: `{}`)\n", p, sub.id));
                    } else {
                        task_out.push_str(&format!("- `{}`\n", p));
                    }
                }
                task_out.push('\n');
            }
        }

        let paginated =
            paginate_content(&task_out, cursor, project.config.max_response_tokens, false);
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
