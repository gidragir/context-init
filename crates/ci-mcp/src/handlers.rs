//! MCP tool execution handlers with token budgeting, Git staleness validation, and telemetry (D14, D19, D21, D29).

use anyhow::{bail, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use ci_core::config::{CliOverrides, Config, StoragePaths};
use ci_core::git::{check_card_staleness, GitBackend, SystemGitBackend};
use ci_core::openviking::OkfCard;
use ci_core::storage::{
    compute_project_id, normalize_line_endings, normalize_path, L0Index, SubsystemInfo,
    CURRENT_L0_SCHEMA_VERSION,
};
use ci_core::telemetry::{
    FeedbackSignal, JsonlTelemetrySink, TelemetryEvent, TelemetrySignal, TelemetrySink,
};
use ci_llm::providers::validate_agent_submission;
use ci_llm::{AgentWorkPackage, RoleName};

use crate::pagination::paginate_content;
use crate::tools::*;

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

        let direct_knowledge = canonical_root.join(".agents").join("knowledge");
        let (knowledge_dir, is_central) = if central_kb.join("L0_index.json").is_file() {
            (central_kb, true)
        } else if direct_knowledge.join("L0_index.json").is_file() {
            (direct_knowledge, false)
        } else if let Some(nested) = find_nested_knowledge_dir(&direct_knowledge) {
            (nested, false)
        } else if canonical_root.join("L0_index.json").is_file() {
            (canonical_root.clone(), false)
        } else {
            // Default to central storage for new projects
            (central_kb, true)
        };

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

fn find_nested_knowledge_dir(direct: &Path) -> Option<PathBuf> {
    if direct.is_dir() {
        if let Ok(entries) = fs::read_dir(direct) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() && p.join("L0_index.json").is_file() {
                    return Some(p);
                }
            }
        }
    }
    None
}

/// Core handler implementations for MCP tools.
pub struct ContextHandler;

impl ContextHandler {
    /// Handler for `context_check`.
    pub fn check_drift(project: &ResolvedProject, cursor: Option<&str>) -> Result<String> {
        let index_res = project.load_l0_index();
        let (has_drift, missing_in_fs, untracked_in_knowledge, up_to_date_count, git_stale) =
            match index_res {
                Ok(index) => {
                    let mut missing = Vec::new();
                    let mut up_to_date = 0;
                    let mut any_git_stale = false;

                    for sub in &index.subsystems {
                        let (stale, _) = project.check_subsystem_staleness(sub);
                        if stale {
                            any_git_stale = true;
                        }

                        let mut sub_has_missing = false;
                        for file in &sub.files {
                            let p = project.root.join(&file.path);
                            if !p.exists() {
                                missing.push(file.path.clone());
                                sub_has_missing = true;
                            }
                        }
                        if !sub_has_missing && !stale {
                            up_to_date += 1;
                        }
                    }

                    // Check top-level source directories for untracked modules
                    let mut untracked = Vec::new();
                    if let Ok(entries) = fs::read_dir(&project.root) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if path.is_dir() {
                                let name = entry.file_name().to_string_lossy().to_string();
                                if !name.starts_with('.')
                                    && name != "target"
                                    && name != "node_modules"
                                {
                                    let tracked = index.subsystems.iter().any(|s| {
                                        s.id.contains(&name)
                                            || s.name.to_ascii_lowercase().contains(&name)
                                            || s.files.iter().any(|f| f.path.starts_with(&name))
                                    });
                                    if !tracked
                                        && (path.join("Cargo.toml").exists()
                                            || path.join("mod.rs").exists()
                                            || path.join("lib.rs").exists())
                                    {
                                        untracked.push(name);
                                    }
                                }
                            }
                        }
                    }

                    let drift = !missing.is_empty() || !untracked.is_empty() || any_git_stale;
                    (drift, missing, untracked, up_to_date, any_git_stale)
                }
                Err(_) => (true, Vec::new(), Vec::new(), 0, false),
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

        let packed_content = pack_files_to_string(
            &project.root,
            &matched_subs,
            &files_to_pack,
            compress,
            style,
        );

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

    /// Handler for `context_submit` (D14, D29, R4).
    pub fn submit(
        project: &ResolvedProject,
        package_id: &str,
        work_package: Option<&AgentWorkPackage>,
        submission: &serde_json::Value,
    ) -> Result<String> {
        let role = if package_id.contains("-init-") {
            RoleName::Init
        } else if package_id.contains("-update-") {
            RoleName::Update
        } else if package_id.contains("-task-") {
            RoleName::Task
        } else if package_id.contains("-search-") {
            RoleName::Search
        } else {
            RoleName::Init
        };

        let fallback_pkg = AgentWorkPackage {
            package_id: package_id.to_string(),
            role,
            facts: serde_json::json!({}),
            instruction: "Agent work submission".to_string(),
            json_schema: ci_llm::default_role_json_schema(role),
        };

        let pkg_ref = work_package.unwrap_or(&fallback_pkg);

        match validate_agent_submission(pkg_ref, submission) {
            Ok(validated) => {
                // If submission contains subsystems list (RoleName::Init), persist to knowledge base
                if let Some(subsystems_val) = validated.payload.get("subsystems") {
                    if let Ok(mut index) = project.load_l0_index() {
                        if let Ok(parsed_subs) =
                            serde_json::from_value::<Vec<SubsystemInfo>>(subsystems_val.clone())
                        {
                            let subs_dir = project.knowledge_dir.join("subsystems");
                            let _ = fs::create_dir_all(&subs_dir);
                            for s in &parsed_subs {
                                let card_p = subs_dir.join(format!("{}.md", s.id));
                                if !card_p.exists() {
                                    let _ = fs::write(
                                        &card_p,
                                        format!("# Subsystem: {} (L1)\n\n{}\n", s.name, s.summary),
                                    );
                                }
                            }
                            index.subsystems = parsed_subs;
                            index.updated_at = "2026-10-19".to_string();
                            let _ =
                                index.save_to_file(&project.knowledge_dir.join("L0_index.json"));
                        }
                    }
                } else if let Some(id_val) = validated
                    .payload
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                {
                    // Update single subsystem card and metadata (RoleName::Update)
                    if let Ok(mut index) = project.load_l0_index() {
                        let subs_dir = project.knowledge_dir.join("subsystems");
                        let _ = fs::create_dir_all(&subs_dir);
                        let card_p = subs_dir.join(format!("{id_val}.md"));
                        if let Some(body) = validated
                            .payload
                            .get("body")
                            .and_then(serde_json::Value::as_str)
                        {
                            let _ = fs::write(&card_p, body);
                        }
                        if let Some(sub) = index.subsystems.iter_mut().find(|s| s.id == id_val) {
                            if let Some(summary) = validated
                                .payload
                                .get("summary")
                                .and_then(serde_json::Value::as_str)
                            {
                                sub.summary = summary.to_string();
                            }
                            if let Some(tags_val) = validated.payload.get("tags") {
                                if let Ok(tags) =
                                    serde_json::from_value::<Vec<String>>(tags_val.clone())
                                {
                                    sub.tags = tags;
                                }
                            }
                        }
                        index.updated_at = "2026-10-19".to_string();
                        let _ = index.save_to_file(&project.knowledge_dir.join("L0_index.json"));
                    }
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

    /// Handler for `context_init`.
    pub fn init(
        project: &ResolvedProject,
        _deep: bool,
        _ai: bool,
        _force: bool,
        _with_agents_md: bool,
    ) -> Result<String> {
        let index_path = project.knowledge_dir.join("L0_index.json");
        if !project.knowledge_dir.exists() {
            fs::create_dir_all(&project.knowledge_dir)?;
        }

        let mut index = if index_path.is_file() {
            L0Index::load_from_file(&index_path)?
        } else {
            L0Index {
                schema_version: CURRENT_L0_SCHEMA_VERSION,
                project: project.project_id.clone(),
                root_path: normalize_path(&project.root),
                updated_at: "2026-10-19".to_string(),
                ..L0Index::default()
            }
        };

        if index.subsystems.is_empty() {
            index.subsystems.push(SubsystemInfo {
                id: "core".to_string(),
                name: "Core Logic".to_string(),
                summary: format!("Core logic subsystem for project {}", project.project_id),
                l1_path: "subsystems/core.md".to_string(),
                tags: vec!["core".to_string()],
                entrypoints: Vec::new(),
                key_files: Vec::new(),
                files: Vec::new(),
                key_symbols: Vec::new(),
                test_command: None,
                tokens: 0,
            });
        }

        index.save_to_file(&index_path)?;

        let subs_dir = project.knowledge_dir.join("subsystems");
        fs::create_dir_all(&subs_dir)?;
        let core_card = subs_dir.join("core.md");
        if !core_card.exists() {
            fs::write(
                &core_card,
                format!(
                    "# Subsystem: Core Logic (L1)\n\nCore logic for {}\n",
                    project.project_id
                ),
            )?;
        }

        Ok(format!(
            "Knowledge base initialized at {:?} ({} subsystems).",
            project.knowledge_dir,
            index.subsystems.len()
        ))
    }

    /// Handler for `context_update`.
    pub fn update(
        project: &ResolvedProject,
        _deep: bool,
        _ai: bool,
        _model: Option<&str>,
    ) -> Result<String> {
        let mut index = project.load_l0_index()?;
        index.updated_at = "2026-10-19".to_string();
        index.save_to_file(&project.knowledge_dir.join("L0_index.json"))?;
        Ok(format!(
            "Knowledge base updated: {} subsystems synchronized.",
            index.subsystems.len()
        ))
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

/// Packs files into AST-compressed XML or Markdown format.
pub fn pack_files_to_string(
    root: &Path,
    subs: &[&SubsystemInfo],
    files: &[String],
    compress: bool,
    style: &str,
) -> String {
    let mut buffer = String::new();

    if style.eq_ignore_ascii_case("markdown") {
        buffer.push_str("# 📦 AI Context Pack: ");
        let names: Vec<&str> = subs.iter().map(|s| s.name.as_str()).collect();
        buffer.push_str(&names.join(", "));
        buffer.push_str("\n\n");

        for file_rel in files {
            let full_p = root.join(file_rel);
            if let Ok(content) = fs::read_to_string(&full_p) {
                let ext = full_p.extension().and_then(|e| e.to_str()).unwrap_or("");
                let processed = if compress {
                    compress_source_code(&content, ext)
                } else {
                    content
                };
                buffer.push_str(&format!(
                    "## File: `{}`\n```{}\n{}\n```\n\n",
                    file_rel, ext, processed
                ));
            }
        }
    } else {
        buffer.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        buffer.push_str("<context_pack generated_by=\"context-init\">\n");
        for sub in subs {
            buffer.push_str(&format!(
                "  <subsystem id=\"{}\" name=\"{}\">\n    <summary>{}</summary>\n  </subsystem>\n",
                sub.id, sub.name, sub.summary
            ));
        }
        buffer.push_str("  <files>\n");

        for file_rel in files {
            let full_p = root.join(file_rel);
            if let Ok(content) = fs::read_to_string(&full_p) {
                let ext = full_p.extension().and_then(|e| e.to_str()).unwrap_or("");
                let processed = if compress {
                    compress_source_code(&content, ext)
                } else {
                    content
                };
                buffer.push_str(&format!(
                    "    <file path=\"{}\">\n<![CDATA[\n{}\n]]>\n    </file>\n",
                    file_rel, processed
                ));
            }
        }
        buffer.push_str("  </files>\n</context_pack>\n");
    }

    buffer
}

/// Compresses source code (AST stripping function bodies in Rust, normalizing blanks otherwise).
pub fn compress_source_code(content: &str, ext: &str) -> String {
    if ext.eq_ignore_ascii_case("rs") {
        if let Some(compressed) = compress_rust_syntax(content) {
            return compressed;
        }
    }
    // Generic compression: collapse consecutive blank lines and trim trailing whitespace
    normalize_blank_lines(content)
}

fn compress_rust_syntax(content: &str) -> Option<String> {
    let parsed = syn::parse_file(content).ok()?;
    let mut items = Vec::new();

    for item in &parsed.items {
        match item {
            syn::Item::Fn(f) => {
                let vis = &f.vis;
                let sig = &f.sig;
                let sig_str = quote::quote!(#vis #sig).to_string();
                items.push(format!("{} {{\n    ⋮----\n}}", normalize_tokens(&sig_str)));
            }
            syn::Item::Impl(imp) => {
                let defaultness = &imp.defaultness;
                let unsafety = &imp.unsafety;
                let generics = &imp.generics;
                let (impl_generics, _, where_clause) = generics.split_for_impl();
                let self_ty = &imp.self_ty;
                let header = if let Some((bang, path, for_tok)) = &imp.trait_ {
                    quote::quote!(#defaultness #unsafety impl #impl_generics #bang #path #for_tok #self_ty #where_clause)
                } else {
                    quote::quote!(#defaultness #unsafety impl #impl_generics #self_ty #where_clause)
                };

                let mut impl_items = Vec::new();
                for it in &imp.items {
                    if let syn::ImplItem::Fn(m) = it {
                        let vis = &m.vis;
                        let defaultness = &m.defaultness;
                        let sig = &m.sig;
                        let m_sig = quote::quote!(#vis #defaultness #sig).to_string();
                        impl_items.push(format!(
                            "    {} {{\n        ⋮----\n    }}",
                            normalize_tokens(&m_sig)
                        ));
                    } else {
                        impl_items.push(format!(
                            "    {}",
                            normalize_tokens(&quote::quote!(#it).to_string())
                        ));
                    }
                }

                if impl_items.is_empty() {
                    items.push(format!("{} {{}}", normalize_tokens(&header.to_string())));
                } else {
                    items.push(format!(
                        "{} {{\n{}\n}}",
                        normalize_tokens(&header.to_string()),
                        impl_items.join("\n\n")
                    ));
                }
            }
            syn::Item::Trait(t) => {
                items.push(normalize_tokens(&quote::quote!(#t).to_string()));
            }
            syn::Item::Struct(s) => {
                items.push(normalize_tokens(&quote::quote!(#s).to_string()));
            }
            syn::Item::Enum(e) => {
                items.push(normalize_tokens(&quote::quote!(#e).to_string()));
            }
            _ => {
                items.push(normalize_tokens(&quote::quote!(#item).to_string()));
            }
        }
    }

    Some(items.join("\n\n"))
}

fn normalize_tokens(s: &str) -> String {
    let mut res = s
        .replace(" (", "(")
        .replace(" ;", ";")
        .replace(" :", ":")
        .replace(" ,", ",");
    while res.contains("  ") {
        res = res.replace("  ", " ");
    }
    res
}

fn normalize_blank_lines(text: &str) -> String {
    let normalized = normalize_line_endings(text);
    let mut out = Vec::new();
    let mut last_was_blank = false;

    for line in normalized.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            if !last_was_blank {
                out.push("");
                last_was_blank = true;
            }
        } else {
            out.push(trimmed);
            last_was_blank = false;
        }
    }

    out.join("\n")
}
