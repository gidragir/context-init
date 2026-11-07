//! MCP tool execution handlers with token budgeting, Git staleness validation, and telemetry (D14, D19, D21, D29).

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ci_core::analysis::generator::{self, SummaryUpdate};
use ci_core::analysis::pack::{pack_subsystem, PackStyle, PackedSubsystems};
use ci_core::analysis::resolve_knowledge_dir;
use ci_core::analysis::scan::{render_scan_markdown, scan_project_with, ScanOptions};
use ci_core::analysis::task::{build_task_digest, TaskDigestRequest};
use ci_core::analysis::workspace::{analyze_workspace, render_workspace_l0};
use ci_core::config::ROLE_NONE;
use ci_core::config::{CliOverrides, Config, StorageMode, StoragePaths};
use ci_core::git::{
    check_card_staleness, check_card_staleness_with_snapshot, GitBackend, RepoGitSnapshot,
    SystemGitBackend,
};
use ci_core::openviking::OkfCard;
use ci_core::search::{Bm25Retriever, Retriever, SearchHit};
use ci_core::storage::{
    compute_project_id, safe_join, subsystem_card_path, subsystem_tokens, L0Index, SubsystemInfo,
};
use ci_core::telemetry::{
    record_feedback, FeedbackSignal, Recorder, TelemetryEvent, TelemetryScope, TelemetrySignal,
    TelemetrySink,
};
use ci_core::SummarySource;
use ci_llm::enrich::{cli_outcome_message, EnrichOutcome};
use ci_llm::providers::validate_agent_submission;
use ci_llm::{AgentWorkPackage, LlmRequest, RoleName, RoleRouter};

use crate::pipeline::{self, PipelineOptions};

use crate::pagination::{count_tokens, decode_cursor, paginate_content, PaginatedOutput};
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

    /// Telemetry recorder for this project: writes only when the project's own
    /// `telemetry.enabled` is set and a sink is available; otherwise a no-op recorder.
    pub fn recorder(&self, sink: Option<Arc<dyn TelemetrySink>>) -> Recorder {
        if self.config.telemetry.enabled {
            Recorder::new(sink, Some(self.project_id.clone()))
        } else {
            Recorder::disabled()
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

    /// Checks if a subsystem card is stale relative to Git using a precomputed snapshot.
    pub fn check_subsystem_staleness_with_snap(
        &self,
        subsystem: &SubsystemInfo,
        snapshot: &RepoGitSnapshot,
        backend: &dyn GitBackend,
    ) -> (bool, Option<String>) {
        let Some(path) = self.card_file(subsystem) else {
            return (false, None);
        };

        let Ok(content) = fs::read_to_string(&path) else {
            return (false, None);
        };

        if let Ok(card) = OkfCard::parse(&content) {
            if let Ok(report) =
                check_card_staleness_with_snapshot(backend, &self.root, snapshot, &card)
            {
                if report.stale {
                    return (true, Some(report.diff_stat));
                }
            }
        }

        (false, None)
    }

    /// Checks if a subsystem card is stale relative to Git.
    pub fn check_subsystem_staleness(&self, subsystem: &SubsystemInfo) -> (bool, Option<String>) {
        let backend = SystemGitBackend::new();
        if let Ok(snapshot) = backend.repo_snapshot(&self.root) {
            self.check_subsystem_staleness_with_snap(subsystem, &snapshot, &backend)
        } else {
            let Some(path) = self.card_file(subsystem) else {
                return (false, None);
            };

            let Ok(content) = fs::read_to_string(&path) else {
                return (false, None);
            };

            if let Ok(card) = OkfCard::parse(&content) {
                if let Ok(report) = check_card_staleness(&backend, &self.root, &card) {
                    if report.stale {
                        return (true, Some(report.diff_stat));
                    }
                }
            }

            (false, None)
        }
    }
}

/// Live source tokens of the listed subsystems. `SubsystemInfo::tokens` is not persisted, so it
/// is filled from the current sources for these ids only, then summed by the storage helper.
fn live_source_tokens(project: &ResolvedProject, index: &L0Index, ids: &[&str]) -> usize {
    let hydrated = L0Index {
        subsystems: index
            .subsystems
            .iter()
            .filter(|s| ids.contains(&s.id.as_str()))
            .cloned()
            .map(|mut s| {
                s.tokens = ci_core::analysis::subsystems::subsystem_tokens(&project.root, &s);
                s
            })
            .collect(),
        ..L0Index::default()
    };
    subsystem_tokens(&hydrated, ids)
}

/// Records the efficiency signal of one paginated reply (no-op when telemetry is inactive).
fn record_page(
    tel: &TelemetryScope,
    tool: &str,
    source_tokens: usize,
    empty_search: bool,
    cursor: Option<&str>,
    page: &PaginatedOutput,
) {
    let hops = cursor.and_then(decode_cursor).map(|c| c.page).unwrap_or(0);
    tel.efficiency(
        tool,
        source_tokens,
        &page.rendered,
        page.truncated,
        hops,
        page.stale,
        empty_search,
    );
}

/// Number of search results returned by default.
pub const DEFAULT_SEARCH_TOP_K: usize = 10;

/// Asks the `search` role to order the candidate ids; `None` when the role is unavailable
/// or its reply is not a usable `{"ranked_ids": [...]}`.
fn rerank_with_llm(router: &RoleRouter, query: &str, hits: &[SearchHit]) -> Option<Vec<String>> {
    let facts = hits
        .iter()
        .map(|h| format!("- {}: {} — {}", h.subsystem_id, h.name, h.summary))
        .collect::<Vec<_>>()
        .join("\n");
    let prompt = format!(
        "Search query: {query}\n\nCandidates:\n{facts}\n\nReturn JSON {{\"ranked_ids\": [candidate ids, most relevant first]}}."
    );
    let req = LlmRequest::new(RoleName::Search, prompt);
    let resp = router.complete_role(&req, None).ok()??;
    let start = resp.content.find('{')?;
    let end = resp.content.rfind('}')?;
    let value: serde_json::Value = serde_json::from_str(&resp.content[start..=end]).ok()?;
    let ids: Vec<String> = value
        .get("ranked_ids")?
        .as_array()?
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .filter(|id| hits.iter().any(|h| &h.subsystem_id == id))
        .collect();
    (!ids.is_empty()).then_some(ids)
}

/// Reply of a handler that may hand an agent work package to the server.
#[derive(Debug, Clone)]
pub struct HandlerReply {
    /// Paginated text returned to the MCP client.
    pub text: String,
    /// Work package the server must register for a later `context_submit`.
    pub pending_package: Option<AgentWorkPackage>,
}

/// Reply of `context_submit`.
#[derive(Debug, Clone)]
pub struct SubmitReply {
    /// Paginated `SubmitResult` JSON returned to the MCP client.
    pub text: String,
    /// Whether the submission was applied; an accepted package is consumed.
    pub accepted: bool,
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
            format_issues,
            ..
        } = pipeline::run_check(project)?;

        let message = if !has_drift {
            format!("OK: {} subsystems verified up-to-date", up_to_date_count)
        } else {
            format!(
                "DRIFT: up_to_date={}, missing=[{}], untracked=[{}], git_stale={}, format=[{}]",
                up_to_date_count,
                missing_in_fs.join(", "),
                untracked_in_knowledge.join(", "),
                git_stale,
                format_issues.join("; ")
            )
        };

        let result = McpCheckResult {
            has_drift,
            missing_in_fs,
            untracked_in_knowledge,
            up_to_date_count,
            git_stale,
            format_issues,
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
    pub fn get_subsystems(
        project: &ResolvedProject,
        cursor: Option<&str>,
        tel: &TelemetryScope,
    ) -> Result<String> {
        let index = project.load_l0_index()?;
        let git = SystemGitBackend::new();
        let snapshot = git.repo_snapshot(&project.root).ok();
        let mut any_stale = false;
        let mut summaries = Vec::with_capacity(index.subsystems.len());
        for sub in &index.subsystems {
            let (is_stale, _) = if let Some(snap) = &snapshot {
                project.check_subsystem_staleness_with_snap(sub, snap, &git)
            } else {
                project.check_subsystem_staleness(sub)
            };
            if is_stale {
                any_stale = true;
            }
            let tokens = ci_core::analysis::subsystems::subsystem_tokens(&project.root, sub);
            summaries.push(L0SubsystemSummary {
                id: sub.id.clone(),
                name: sub.name.clone(),
                parent: sub.parent.clone(),
                summary: sub.summary.clone(),
                tags: sub.tags.clone(),
                file_count: sub.files.len(),
                tokens,
                stale: is_stale,
                needs_review: sub.needs_review,
            });
        }

        let json_text = if summaries.is_empty() {
            "[]".to_string()
        } else {
            let mut buf = String::from("[\n");
            for (idx, entry) in summaries.iter().enumerate() {
                buf.push_str("  ");
                buf.push_str(&serde_json::to_string(entry)?);
                if idx + 1 < summaries.len() {
                    buf.push(',');
                }
                buf.push('\n');
            }
            buf.push(']');
            buf
        };

        let paginated = paginate_content(
            &json_text,
            cursor,
            project.config.max_response_tokens,
            any_stale,
        );
        if tel.is_active() {
            let ids: Vec<&str> = index.subsystems.iter().map(|s| s.id.as_str()).collect();
            let source = live_source_tokens(project, &index, &ids);
            record_page(
                tel,
                "context_get_subsystems",
                source,
                false,
                cursor,
                &paginated,
            );
        }
        Ok(paginated.rendered)
    }

    /// Handler for `context_get_subsystem`.
    pub fn get_subsystem(
        project: &ResolvedProject,
        subsystem_id: &str,
        cursor: Option<&str>,
        tel: &TelemetryScope,
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
                let paginated =
                    paginate_content(&card, cursor, project.config.max_response_tokens, false);
                record_page(tel, "context_get_subsystem", 0, false, cursor, &paginated);
                return Ok(paginated.rendered);
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
        if tel.is_active() {
            let source = live_source_tokens(project, &index, &[sub.id.as_str()]);
            record_page(
                tel,
                "context_get_subsystem",
                source,
                false,
                cursor,
                &paginated,
            );
        }
        Ok(paginated.rendered)
    }

    /// Handler for `context_pack`.
    pub fn pack(
        project: &ResolvedProject,
        subsystem_id: &str,
        compress: bool,
        style: &str,
        cursor: Option<&str>,
        tel: &TelemetryScope,
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
        let source_tokens = if tel.is_active() {
            let ids: Vec<&str> = matched_subs.iter().map(|s| s.id.as_str()).collect();
            live_source_tokens(project, &index, &ids)
        } else {
            0
        };
        if subsystem_id.eq_ignore_ascii_case("all")
            && count_tokens(&packed_content) > PACK_ALL_MAX_PAGES.saturating_mul(budget)
        {
            let ids: Vec<&str> = index.subsystems.iter().map(|s| s.id.as_str()).collect();
            let refusal = format!(
                "Refusing pack all: the packed project exceeds {PACK_ALL_MAX_PAGES} pages of {budget} tokens (D19). \
                 Pack per subsystem instead with context_pack(subsystem_id=<id>). Subsystems: {}",
                ids.join(", ")
            );
            let paginated = paginate_content(&refusal, cursor, budget, false);
            record_page(
                tel,
                "context_pack",
                source_tokens,
                false,
                cursor,
                &paginated,
            );
            return Ok(paginated.rendered);
        }

        let git = SystemGitBackend::new();
        let snapshot = git.repo_snapshot(&project.root).ok();
        let any_stale = matched_subs.iter().any(|s| {
            if let Some(snap) = &snapshot {
                project.check_subsystem_staleness_with_snap(s, snap, &git).0
            } else {
                project.check_subsystem_staleness(s).0
            }
        });

        let paginated = paginate_content(
            &packed_content,
            cursor,
            project.config.max_response_tokens,
            any_stale,
        );
        record_page(
            tel,
            "context_pack",
            source_tokens,
            false,
            cursor,
            &paginated,
        );
        Ok(paginated.rendered)
    }

    /// Applies a validated agent submission to the whole knowledge base.
    ///
    /// Only the `{"subsystems": [{id, name, summary, tags}]}` form is accepted. The project is
    /// re-analyzed deterministically (at the package's depth) and the summaries are merged via
    /// [`generator::apply_summaries`] with `agent` provenance and the package's model alias, which
    /// re-renders the affected cards, `L0_index.md`, `L0_index.json` and (with
    /// `touch_agents_md`) `AGENTS.md`. The call is all-or-nothing: an invalid or unknown id, or a
    /// failed analysis, leaves every file untouched. File lists are never taken from
    /// submissions: they come from the detector only. Returns the number of subsystems updated.
    fn persist_submission(
        project: &ResolvedProject,
        pkg: &AgentWorkPackage,
        payload: &serde_json::Value,
    ) -> Result<usize> {
        let kd = &project.knowledge_dir;
        let items = payload
            .get("subsystems")
            .and_then(serde_json::Value::as_array)
            .context(
                "Submission must have the form {\"subsystems\": [{\"id\", \"name\", \"summary\", \"tags\"}]}; \
                 single cards with `body` are not accepted",
            )?;
        let str_field = |v: &serde_json::Value, key: &str| {
            v.get(key)
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned)
        };

        let mut updates = Vec::with_capacity(items.len());
        for item in items {
            let id = str_field(item, "id").unwrap_or_default();
            // Rejects traversal and absolute ids with an explicit error before any analysis.
            subsystem_card_path(kd, &id)?;
            let summary = str_field(item, "summary")
                .filter(|s| !s.trim().is_empty())
                .with_context(|| format!("Subsystem '{id}': summary must not be empty"))?;
            updates.push(SummaryUpdate {
                name: str_field(item, "name").filter(|n| !n.trim().is_empty()),
                // An empty list keeps the existing tags, as in LLM enrichment.
                tags: item
                    .get("tags")
                    .and_then(|t| serde_json::from_value::<Vec<String>>(t.clone()).ok())
                    .filter(|t| !t.is_empty()),
                id,
                summary,
            });
        }

        let analysis = pipeline::detect(project, pkg.deep)
            .context("Re-analysis of the project failed; the knowledge base is unchanged")?;
        let model = (!pkg.alias.is_empty()).then_some(pkg.alias.as_str());
        // Refresh AGENTS.md when the originating init/update wrote it, or the project asks to.
        let agents_md =
            (pkg.agents_md || project.config.touch_agents_md).then_some(project.storage);
        generator::apply_summaries(
            &analysis,
            kd,
            &updates,
            SummarySource::Agent,
            model,
            agents_md,
        )
    }

    /// Handler for `context_submit` (D14, D29, R4). `accepted` tells the server whether the
    /// package was consumed (packages are single-use).
    pub fn submit(
        project: &ResolvedProject,
        package_id: &str,
        work_package: Option<&AgentWorkPackage>,
        submission: &serde_json::Value,
    ) -> Result<SubmitReply> {
        let result = Self::submit_result(project, package_id, work_package, submission);
        let json_text = serde_json::to_string_pretty(&result)?;
        Ok(SubmitReply {
            text: paginate_content(&json_text, None, project.config.max_response_tokens, false)
                .rendered,
            accepted: result.valid,
        })
    }

    fn submit_result(
        project: &ResolvedProject,
        package_id: &str,
        work_package: Option<&AgentWorkPackage>,
        submission: &serde_json::Value,
    ) -> SubmitResult {
        let rejected = |role: String, errors: Vec<String>, message: &str| SubmitResult {
            valid: false,
            package_id: package_id.to_string(),
            role,
            errors,
            message: message.to_string(),
        };
        let Some(pkg) = work_package else {
            return rejected(
                String::new(),
                vec![format!(
                    "Unknown package_id '{package_id}': request a work package via context_init/context_update first"
                )],
                "Agent submission rejected: unknown work package.",
            );
        };
        if pkg.project_root.as_deref() != Some(project.root.as_path()) {
            let issued_for = pkg
                .project_root
                .as_ref()
                .map_or_else(|| "no project".to_string(), |r| r.display().to_string());
            return rejected(
                pkg.role.to_string(),
                vec![format!(
                    "Work package '{package_id}' was issued for {issued_for}, not for {}",
                    project.root.display()
                )],
                "Agent submission rejected: the work package belongs to another project.",
            );
        }

        let validated = match validate_agent_submission(pkg, submission) {
            Ok(validated) => validated,
            Err(e) => {
                let mut errors = Vec::new();
                for cause in e.chain() {
                    let s = cause.to_string();
                    if !errors.contains(&s) {
                        errors.push(s);
                    }
                }
                return rejected(
                    pkg.role.to_string(),
                    errors,
                    "Agent submission rejected: JSON schema validation failed.",
                );
            }
        };
        if let Err(e) = Self::persist_submission(project, pkg, &validated.payload) {
            return rejected(
                validated.role.to_string(),
                e.chain().map(|c| c.to_string()).collect(),
                "Agent submission rejected: it could not be persisted.",
            );
        }
        SubmitResult {
            valid: true,
            package_id: validated.package_id,
            role: validated.role.to_string(),
            errors: Vec::new(),
            message: "Agent submission successfully validated and accepted.".to_string(),
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
    pub fn scan(
        project: &ResolvedProject,
        cursor: Option<&str>,
        opts: &ScanOptions,
    ) -> Result<String> {
        let report = scan_project_with(&project.root, &project.config.scope, opts)?;
        let scan_out = render_scan_markdown(&project.project_id, &project.root, &report);
        let paginated =
            paginate_content(&scan_out, cursor, project.config.max_response_tokens, false);
        Ok(paginated.rendered)
    }

    /// Handler for `context_workspace` (D6): the L0 map of a directory of Git repositories.
    pub fn workspace(root: &Path, max_tokens: usize, cursor: Option<&str>) -> Result<String> {
        let report = analyze_workspace(root, &SystemGitBackend::new())?;
        let md = render_workspace_l0(&report);
        Ok(paginate_content(&md, cursor, max_tokens, false).rendered)
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
        if out.nothing_to_enrich {
            text.push_str("\nОбогащение не требуется: изменений нет.");
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
        tel: &TelemetryScope,
    ) -> Result<String> {
        let index = project.load_l0_index().ok();
        let digest = build_task_digest(
            &project.root,
            &SystemGitBackend::new(),
            index.as_ref(),
            &TaskDigestRequest {
                paths: paths.unwrap_or(&[]),
                base_ref,
                target,
                max_tokens: project.config.max_response_tokens,
            },
        )?;
        let paginated =
            paginate_content(&digest, cursor, project.config.max_response_tokens, false);
        if let (true, Some(index)) = (tel.is_active(), index.as_ref()) {
            // The digest tags each affected path with its subsystem id.
            let affected: Vec<&str> = index
                .subsystems
                .iter()
                .map(|s| s.id.as_str())
                .filter(|id| digest.contains(&format!("(subsystem: `{id}`)")))
                .collect();
            let source = live_source_tokens(project, index, &affected);
            record_page(tel, "context_task", source, false, cursor, &paginated);
        }
        Ok(paginated.rendered)
    }

    /// Handler for `context_search` (D18): BM25 retrieval, optionally reranked by the `search`
    /// role; any LLM failure or unusable reply keeps the BM25 order.
    pub fn search(
        project: &ResolvedProject,
        query: &str,
        cursor: Option<&str>,
        router: &RoleRouter,
        tel: &TelemetryScope,
    ) -> Result<String> {
        Self::search_top_k(project, query, cursor, router, DEFAULT_SEARCH_TOP_K, tel)
    }

    /// [`ContextHandler::search`] with an explicit result count.
    pub fn search_top_k(
        project: &ResolvedProject,
        query: &str,
        cursor: Option<&str>,
        router: &RoleRouter,
        top_k: usize,
        tel: &TelemetryScope,
    ) -> Result<String> {
        let index = project.load_l0_index()?;
        let retriever = Bm25Retriever::build(&index, &project.knowledge_dir)?;
        let mut hits = retriever.search(query, top_k)?;
        let budget = project.config.max_response_tokens;
        let render = |hits: &[SearchHit]| -> String {
            let mut out = format!(
                "# Search Results for '{}'\n\nFound {} matching subsystems:\n\n",
                query.split_whitespace().collect::<Vec<_>>().join(" "),
                hits.len()
            );
            for h in hits {
                let tags = index
                    .subsystems
                    .iter()
                    .find(|s| s.id == h.subsystem_id)
                    .map(|s| s.tags.join(", "))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "### `{}` ({})\n{}\n> **Score:** {:.2} | **Tags:** {}",
                    h.subsystem_id, h.name, h.summary, h.score, tags
                ));
                if !h.matched_symbols.is_empty() {
                    out.push_str(&format!(" | **Symbols:** {}", h.matched_symbols.join(", ")));
                }
                out.push_str("\n\n");
            }
            out
        };
        let chain = &project.config.roles.search;
        let llm_enabled = !(chain.len() == 1 && chain[0].trim().eq_ignore_ascii_case(ROLE_NONE));
        // Rerank only when the whole result is a single page: a model may order differently
        // per call, which would duplicate or skip results across pages.
        if llm_enabled
            && cursor.is_none()
            && hits.len() > 1
            && !paginate_content(&render(&hits), None, budget, false).truncated
        {
            if let Some(order) = rerank_with_llm(router, query, &hits) {
                hits.sort_by_key(|h| {
                    order
                        .iter()
                        .position(|id| *id == h.subsystem_id)
                        .unwrap_or(usize::MAX)
                });
            }
        }
        let out = render(&hits);
        let paginated = paginate_content(&out, cursor, budget, false);
        if tel.is_active() {
            let ids: Vec<&str> = hits.iter().map(|h| h.subsystem_id.as_str()).collect();
            let source = live_source_tokens(project, &index, &ids);
            record_page(
                tel,
                "context_search",
                source,
                hits.is_empty(),
                cursor,
                &paginated,
            );
        }
        Ok(paginated.rendered)
    }
}
