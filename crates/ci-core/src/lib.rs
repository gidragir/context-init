//! Core types, configuration hierarchy, central storage, Git backend, and telemetry for `context-init`.

pub mod analysis;
pub mod config;
pub mod git;
pub mod openviking;
pub mod storage;
pub mod telemetry;

pub use config::{
    generate_template_toml, is_binary_available, is_binary_available_in_dirs,
    is_binary_available_in_path, is_rtk_available, join_search_paths, resolve_rtk_active,
    resolve_rtk_active_with_path, AppConfig, CliOverrides, Config, IntegrationsConfig, ModelConfig,
    ModelKind, RoleKind, RolesConfig, RtkMode, StorageMode, StoragePaths, TelemetryConfig,
    DEFAULT_LANGUAGE, DEFAULT_MAX_RESPONSE_TOKENS, ROLE_NONE,
};
pub use git::{
    check_card_staleness, check_frontmatter_staleness, parse_nul_paths, parse_porcelain_z,
    GitBackend, GitStatusEntry, StalenessReport, SystemGitBackend,
};
pub use openviking::{
    extract_l0_abstract, extract_l0_abstract_with_limit, CardFreshness, CardFrontmatter,
    CardGeneratedBy, CardSource, OkfCard, L0_ABSTRACT_MAX_CHARS,
};
pub use storage::{
    compute_project_id, hash_project_id, normalize_git_remote, normalize_line_endings,
    normalize_path, normalize_path_str, DriftReport, L0Index, ProjectCommands, ProjectType,
    SubsystemFile, SubsystemInfo, CURRENT_L0_SCHEMA_VERSION,
};
pub use telemetry::{
    read_jsonl_file, record_feedback, render_telemetry_report, EfficiencySignal, FeedbackSignal,
    JsonlTelemetrySink, LlmSignal, OperationalSignal, SessionPatternSignal, TelemetryEvent,
    TelemetrySignal, TelemetrySink, TELEMETRY_SCHEMA_VERSION,
};

/// Current crate version synchronized with the workspace package metadata.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns the crate identifier and version string.
pub fn crate_info() -> &'static str {
    concat!("ci-core/", env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(1);

    struct TempDirGuard {
        path: PathBuf,
    }

    impl TempDirGuard {
        fn new(prefix: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let seq = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let pid = std::process::id();
            let path = std::env::temp_dir().join(format!("ci_core_{prefix}_{pid}_{nanos}_{seq}"));
            fs::create_dir_all(&path).expect("create temp test dir");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn smoke_version_matches_workspace() {
        assert_eq!(VERSION, "0.3.0");
        assert!(crate_info().starts_with("ci-core/"));
    }

    #[test]
    fn storage_paths_xdg_and_defaults_and_appdata_fallback() {
        let tmp = TempDirGuard::new("paths");
        let home = tmp.path().join("home");
        let repo = tmp.path().join("repo");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&repo).unwrap();

        // 1. Default paths when XDG_* are unset/empty
        let paths_default = StoragePaths::resolve_with_env(Some(&repo), Some(&home), |_| None);
        assert_eq!(
            paths_default.global_config_path,
            home.join(".config")
                .join("context-init")
                .join("config.toml")
        );
        assert_eq!(
            paths_default.project_config_path,
            Some(repo.join(".context-init.toml"))
        );
        assert_eq!(
            paths_default.data_dir,
            home.join(".local").join("share").join("context-init")
        );
        assert_eq!(
            paths_default.project_storage_dir("github.com_org_app-12345678"),
            home.join(".local")
                .join("share")
                .join("context-init")
                .join("projects")
                .join("github.com_org_app-12345678")
        );
        assert_eq!(
            paths_default.cache_dir,
            home.join(".cache").join("context-init")
        );

        // 2. Custom XDG variables override defaults
        let xdg_cfg = tmp.path().join("xdg_config");
        let xdg_data = tmp.path().join("xdg_data");
        let xdg_cache = tmp.path().join("xdg_cache");
        let mut env_map = BTreeMap::new();
        env_map.insert(
            "XDG_CONFIG_HOME".to_string(),
            xdg_cfg.to_string_lossy().to_string(),
        );
        env_map.insert(
            "XDG_DATA_HOME".to_string(),
            xdg_data.to_string_lossy().to_string(),
        );
        env_map.insert(
            "XDG_CACHE_HOME".to_string(),
            xdg_cache.to_string_lossy().to_string(),
        );

        let paths_xdg =
            StoragePaths::resolve_with_env(Some(&repo), Some(&home), |k| env_map.get(k).cloned());
        assert_eq!(
            paths_xdg.global_config_path,
            xdg_cfg.join("context-init").join("config.toml")
        );
        assert_eq!(paths_xdg.data_dir, xdg_data.join("context-init"));
        assert_eq!(
            paths_xdg.projects_dir,
            xdg_data.join("context-init").join("projects")
        );
        assert_eq!(paths_xdg.cache_dir, xdg_cache.join("context-init"));

        // 3. Windows %APPDATA% fallback when primary ~/.config/context-init/config.toml does not exist
        let appdata = tmp.path().join("AppData").join("Roaming");
        let appdata_cfg = appdata.join("context-init").join("config.toml");
        fs::create_dir_all(appdata_cfg.parent().unwrap()).unwrap();
        fs::write(&appdata_cfg, "language = \"en\"\n").unwrap();

        let mut win_env = BTreeMap::new();
        win_env.insert("APPDATA".to_string(), appdata.to_string_lossy().to_string());
        let paths_win =
            StoragePaths::resolve_with_env(Some(&repo), Some(&home), |k| win_env.get(k).cloned());
        assert_eq!(
            paths_win.effective_global_config_path(),
            appdata_cfg.as_path()
        );
    }

    #[test]
    fn config_priority_cascade_and_overrides() {
        let global_toml = r#"
language = "en"
max_response_tokens = 3000
touch_agents_md = false

[integrations]
rtk = "off"
mise = true
stow = false

[models.local]
kind = "ollama"
model = "qwen2.5:14b-ctx64k"
endpoint = "http://127.0.0.1:11434"

[models.cli_helper]
kind = "command"
command = ["claude", "-p"]

[roles]
init = ["local"]
update = ["local"]
task = ["local"]
search = ["none"]

[telemetry]
enabled = false
"#;

        let project_toml = r#"
language = "de"
max_response_tokens = 2500

[integrations]
rtk = "on"
stow = true

[models.agent_worker]
kind = "agent"

[roles]
init = ["cli_helper", "local"]
"#;

        let mut env_map = BTreeMap::new();
        env_map.insert("CONTEXT_INIT_LANGUAGE".to_string(), "fr".to_string());
        env_map.insert(
            "CONTEXT_INIT_MAX_RESPONSE_TOKENS".to_string(),
            "1800".to_string(),
        );
        env_map.insert(
            "CONTEXT_INIT_ROLE_UPDATE".to_string(),
            "agent_worker,local".to_string(),
        );
        env_map.insert(
            "CONTEXT_INIT_TELEMETRY_ENABLED".to_string(),
            "true".to_string(),
        );

        let cli = CliOverrides {
            language: Some("ru".to_string()),
            touch_agents_md: Some(true),
            ..CliOverrides::default()
        }
        .with_role_override("task=agent_worker,none")
        .unwrap();

        let cfg = Config::from_layers(
            Some(global_toml),
            Some(project_toml),
            |k| env_map.get(k).cloned(),
            &cli,
        )
        .expect("cascade config load");

        // CLI wins over ENV, project, global, default
        assert_eq!(cfg.language, "ru");
        assert!(cfg.touch_agents_md);
        // ENV wins over project and global
        assert_eq!(cfg.max_response_tokens, 1800);
        assert!(cfg.telemetry.enabled);
        assert_eq!(
            cfg.roles.update,
            vec!["agent_worker".to_string(), "local".to_string()]
        );
        // Project wins over global
        assert_eq!(cfg.integrations.rtk, RtkMode::On);
        assert!(cfg.integrations.stow);
        assert_eq!(
            cfg.roles.init,
            vec!["cli_helper".to_string(), "local".to_string()]
        );
        // Global preserved when not overridden in project/ENV/CLI
        assert!(cfg.integrations.mise);
        assert_eq!(cfg.roles.search, vec!["none".to_string()]);
        // CLI role override wins for task
        assert_eq!(
            cfg.roles.task,
            vec!["agent_worker".to_string(), "none".to_string()]
        );
        // Models merged across global + project
        assert!(cfg.models.contains_key("local"));
        assert!(cfg.models.contains_key("cli_helper"));
        assert!(cfg.models.contains_key("agent_worker"));

        cfg.validate().expect("merged config must be valid");
    }

    #[test]
    fn config_cli_model_and_role_override_and_validation() {
        let mut cfg = Config::default();
        cfg.models.insert(
            "fast".to_string(),
            ModelConfig {
                kind: ModelKind::Ollama,
                model: Some("fast-model".to_string()),
                endpoint: None,
                command: None,
                timeout_ms: Some(5000),
                max_input_tokens: Some(8000),
            },
        );
        cfg.models.insert(
            "smart".to_string(),
            ModelConfig {
                kind: ModelKind::Agent,
                model: None,
                endpoint: None,
                command: None,
                timeout_ms: None,
                max_input_tokens: None,
            },
        );

        let cli = CliOverrides::default()
            .with_model("fast")
            .with_role_override("init=smart,fast")
            .unwrap();

        cfg.apply_cli_overrides(&cli).unwrap();
        assert_eq!(
            cfg.roles.init,
            vec!["smart".to_string(), "fast".to_string()]
        );
        assert_eq!(cfg.roles.update, vec!["fast".to_string()]);
        assert_eq!(cfg.roles.task, vec!["fast".to_string()]);
        assert_eq!(cfg.roles.search, vec!["fast".to_string()]);
        cfg.validate().unwrap();

        // Unknown alias must fail validation
        let mut invalid_role_cfg = cfg.clone();
        invalid_role_cfg.roles.search = vec!["nonexistent_alias".to_string()];
        assert!(invalid_role_cfg.validate().is_err());

        // Zero max_response_tokens must fail validation
        let mut zero_tokens_cfg = cfg.clone();
        zero_tokens_cfg.max_response_tokens = 0;
        assert!(zero_tokens_cfg.validate().is_err());

        // Template config must parse and validate cleanly
        let tmp = TempDirGuard::new("cfg_init");
        let cfg_file = tmp.path().join("config.toml");
        Config::init_config_file(&cfg_file, false).unwrap();
        assert!(Config::init_config_file(&cfg_file, false).is_err());
        Config::init_config_file(&cfg_file, true).unwrap();

        let loaded_from_template = Config::from_layers(
            Some(&fs::read_to_string(&cfg_file).unwrap()),
            None,
            |_| None,
            &CliOverrides::default(),
        )
        .unwrap();
        loaded_from_template.validate().unwrap();
        let shown = loaded_from_template.show_toml().unwrap();
        assert!(shown.contains("max_response_tokens = 4000"));
    }

    #[test]
    fn binary_detection_supports_unix_and_windows_exe() {
        let tmp = TempDirGuard::new("bin_detect");
        let dir_empty = tmp.path().join("empty_bin");
        let dir_win = tmp.path().join("win_bin");
        let dir_unix = tmp.path().join("unix_bin");
        fs::create_dir_all(&dir_empty).unwrap();
        fs::create_dir_all(&dir_win).unwrap();
        fs::create_dir_all(&dir_unix).unwrap();

        // Create rtk.exe in dir_win
        fs::write(dir_win.join("rtk.exe"), b"fake exe").unwrap();
        // Create rtk in dir_unix
        fs::write(dir_unix.join("rtk"), b"fake bin").unwrap();

        let empty_path = join_search_paths([&dir_empty]).unwrap();
        let win_path = join_search_paths([&dir_empty, &dir_win]).unwrap();
        let unix_path = join_search_paths([&dir_unix]).unwrap();

        assert!(!is_binary_available_in_path("rtk", Some(&empty_path)));
        assert!(is_binary_available_in_path("rtk", Some(&win_path)));
        assert!(is_binary_available_in_path("rtk.exe", Some(&win_path)));
        assert!(is_binary_available_in_path("rtk", Some(&unix_path)));

        assert!(!resolve_rtk_active_with_path(
            RtkMode::Auto,
            Some(&empty_path)
        ));
        assert!(resolve_rtk_active_with_path(RtkMode::Auto, Some(&win_path)));
        assert!(resolve_rtk_active_with_path(RtkMode::On, Some(&empty_path)));
        assert!(!resolve_rtk_active_with_path(RtkMode::Off, Some(&win_path)));
    }

    #[test]
    fn project_id_normalization_and_sha256_anonymization() {
        let r1 = normalize_git_remote(
            "https://oauth2:ghp_secretToken123@github.com/Gidragir/Context-Init.git/",
        );
        assert_eq!(r1, "github.com/gidragir/context-init");
        assert!(!r1.contains("ghp_secretToken123"));
        assert!(!r1.ends_with(".git"));

        let r2 = normalize_git_remote("git@github.com:Gidragir/Context-Init.git");
        assert_eq!(r2, "github.com/gidragir/context-init");

        let r3 = normalize_git_remote("ssh://git@gitlab.example.org:2222/team/service.git");
        assert_eq!(r3, "gitlab.example.org/team/service");

        let tmp = TempDirGuard::new("proj_id");
        let pid_https = compute_project_id(
            tmp.path(),
            Some("https://user:pass@github.com/org/repo.git"),
        );
        let pid_ssh = compute_project_id(tmp.path(), Some("git@github.com:org/repo.git"));
        assert_eq!(pid_https, pid_ssh);
        assert!(pid_https.starts_with("github.com_org_repo-"));
        assert!(!pid_https.contains("user"));
        assert!(!pid_https.contains("pass"));

        let hashed = hash_project_id(&pid_https);
        assert_eq!(hashed.len(), 64);
        assert!(hashed.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(!hashed.contains("github"));
    }

    #[test]
    fn okf_card_frontmatter_roundtrip_crlf_and_l0_abstract() {
        let crlf_card = "---\r\ndirectory: src\\core\\engine/\r\nsource:\r\n  kind: git\r\n  blob_sha: 1111222233334444\r\n  commit_sha: aaaabbbbccccdddd\r\ngenerated_by:\r\n  component: context-init\r\n  role: init\r\n  model: local\r\nfreshness:\r\n  total_entries: 12\r\n  sampled_entries: 10\r\n  unsampled_entries: 2\r\n  pending_child_changes: 1\r\n---\r\n\r\n# Заголовок подсистемы\r\n\r\nКраткое описание сущности в две строки\r\nдля проверки извлечения первого параграфа.\r\n\r\n## Quick Navigation\r\n- `run`: точка входа\r\n";

        let card = OkfCard::parse(crlf_card).expect("parse CRLF OKF card");
        assert_eq!(card.frontmatter.directory, "src/core/engine");
        assert_eq!(card.frontmatter.source.kind, "git");
        assert_eq!(card.frontmatter.source.blob_sha, "1111222233334444");
        assert_eq!(card.frontmatter.source.commit_sha, "aaaabbbbccccdddd");
        assert_eq!(card.frontmatter.generated_by.component, "context-init");
        assert_eq!(card.frontmatter.generated_by.role, "init");
        assert_eq!(card.frontmatter.generated_by.model, "local");
        assert_eq!(card.frontmatter.freshness.total_entries, 12);
        assert_eq!(card.frontmatter.freshness.sampled_entries, 10);
        assert_eq!(card.frontmatter.freshness.unsampled_entries, 2);
        assert_eq!(card.frontmatter.freshness.pending_child_changes, 1);

        let abstract_text = card.l0_abstract();
        assert_eq!(
            abstract_text,
            "Краткое описание сущности в две строки для проверки извлечения первого параграфа."
        );

        // Roundtrip serialization
        let serialized = card.to_markdown();
        let reparsed = OkfCard::parse(&serialized).expect("reparse serialized OKF card");
        assert_eq!(card, reparsed);

        // Long paragraph truncation to <= 256 chars
        let long_para = format!("# Title\n\n{}\n\nSecond paragraph.", "АБВГД ".repeat(80));
        let truncated = extract_l0_abstract(&long_para);
        assert!(truncated.chars().count() <= L0_ABSTRACT_MAX_CHARS);
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn system_git_backend_and_card_staleness_on_temp_and_current_repo() {
        let backend = SystemGitBackend::new();

        // 1. Verify against current workspace repository
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root");
        let ws_head = backend.head_commit(workspace_root).expect("workspace head");
        assert!(ws_head.len() >= 40);
        let _ws_status = backend
            .status_porcelain_z(workspace_root)
            .expect("workspace status");

        // 2. Verify full lifecycle on an isolated temporary Git repository
        let tmp = TempDirGuard::new("git_repo");
        let repo = tmp.path();

        let run = |args: &[&str]| {
            let out = Command::new("git")
                .arg("-C")
                .arg(repo)
                .args(args)
                .output()
                .expect("git command");
            assert!(
                out.status.success(),
                "git {:?} failed: {}",
                args,
                String::from_utf8_lossy(&out.stderr)
            );
        };

        run(&["init"]);
        run(&["config", "user.email", "ci@example.com"]);
        run(&["config", "user.name", "CI Tester"]);
        run(&[
            "remote",
            "add",
            "origin",
            "https://token@github.com/org/sample.git",
        ]);

        let src_dir = repo.join("src");
        let docs_dir = repo.join("docs");
        fs::create_dir_all(&src_dir).unwrap();
        fs::create_dir_all(&docs_dir).unwrap();
        fs::write(src_dir.join("lib.rs"), "pub fn v1() -> u32 { 1 }\n").unwrap();
        fs::write(docs_dir.join("guide.md"), "# Guide\n").unwrap();
        run(&["add", "."]);
        run(&["commit", "-m", "initial commit"]);

        let c1 = backend.head_commit(repo).unwrap();
        let remote = backend.remote_url(repo).unwrap();
        assert_eq!(
            remote.as_deref(),
            Some("https://token@github.com/org/sample.git")
        );

        let blob1 = backend
            .blob_sha(repo, Path::new("src"))
            .unwrap()
            .expect("tree sha for src");
        let docs_blob1 = backend
            .blob_sha(repo, Path::new("docs"))
            .unwrap()
            .expect("tree sha for docs");

        let card = OkfCard::new(
            CardFrontmatter {
                directory: "src".to_string(),
                source: CardSource {
                    kind: "git".to_string(),
                    blob_sha: blob1.clone(),
                    commit_sha: c1.clone(),
                },
                generated_by: CardGeneratedBy {
                    component: "context-init".to_string(),
                    role: "init".to_string(),
                    model: "none".to_string(),
                },
                freshness: CardFreshness {
                    total_entries: 1,
                    sampled_entries: 1,
                    unsampled_entries: 0,
                    pending_child_changes: 0,
                },
            },
            "# Source Subsystem\n\nInitial version.\n",
        );

        let docs_card = OkfCard::new(
            CardFrontmatter {
                directory: "docs".to_string(),
                source: CardSource {
                    kind: "git".to_string(),
                    blob_sha: docs_blob1,
                    commit_sha: c1.clone(),
                },
                generated_by: CardGeneratedBy {
                    component: "context-init".to_string(),
                    role: "init".to_string(),
                    model: "none".to_string(),
                },
                freshness: CardFreshness {
                    total_entries: 1,
                    sampled_entries: 1,
                    unsampled_entries: 0,
                    pending_child_changes: 0,
                },
            },
            "# Docs Subsystem\n\nUnchanged guide.\n",
        );

        // Fresh state: stale == false
        let fresh_report = check_card_staleness(&backend, repo, &card).unwrap();
        assert!(!fresh_report.stale);
        assert!(fresh_report.changed_files.is_empty());

        // Modify file in src/ and commit c2 -> src card becomes stale, docs card stays fresh
        fs::write(
            src_dir.join("lib.rs"),
            "pub fn v1() -> u32 { 2 }\npub fn v2() {}\n",
        )
        .unwrap();
        run(&["add", "."]);
        run(&["commit", "-m", "second commit"]);

        let c2 = backend.head_commit(repo).unwrap();
        assert_ne!(c1, c2);

        let diff_names = backend.diff_name_only(repo, &c1, &c2).unwrap();
        assert_eq!(diff_names, vec!["src/lib.rs".to_string()]);

        let stale_report = check_card_staleness(&backend, repo, &card).unwrap();
        assert!(stale_report.stale);
        assert_eq!(stale_report.changed_files, vec!["src/lib.rs".to_string()]);
        assert!(stale_report.diff_stat.contains("src/lib.rs"));

        let docs_report = check_card_staleness(&backend, repo, &docs_card).unwrap();
        assert!(!docs_report.stale);
        assert!(docs_report.changed_files.is_empty());

        // Verify porcelain -z parser on untracked & modified files
        fs::write(src_dir.join("new_file.rs"), "// untracked\n").unwrap();
        let status = backend.status_porcelain_z(repo).unwrap();
        assert_eq!(status.len(), 1);
        assert_eq!(status[0].path, "src/new_file.rs");
        assert_eq!(status[0].status_code(), "??");
    }

    #[test]
    fn telemetry_sink_opt_in_jsonl_and_note_redaction() {
        let tmp = TempDirGuard::new("telemetry");
        let data_dir = tmp.path().join("data");

        let ev_op = TelemetryEvent::with_timestamp(
            "github.com_org_repo-12345678",
            "2026-10-19T10:00:00Z",
            TelemetrySignal::Operational(OperationalSignal {
                tool: "context_pack".to_string(),
                channel: "mcp".to_string(),
                harness: Some("antigravity".to_string()),
                latency_ms: 42,
                success: true,
                error_class: None,
                version: VERSION.to_string(),
                os: "linux".to_string(),
            }),
        );

        let ev_fb = TelemetryEvent::with_timestamp(
            "github.com_org_repo-12345678",
            "2026-10-19T10:05:00Z",
            TelemetrySignal::Feedback(FeedbackSignal {
                tool: "context_pack".to_string(),
                verdict: "helpful".to_string(),
                note: Some("CONFIDENTIAL_INTERNAL_MODULE_NAME".to_string()),
            }),
        );

        // 1. Disabled sink must NOT create files or directories
        let disabled_sink = JsonlTelemetrySink::new(&data_dir, false);
        assert!(!disabled_sink.is_enabled());
        disabled_sink.record(&ev_op).unwrap();
        assert!(!disabled_sink.telemetry_dir().exists());

        // 2. Enabled sink writes to <data_dir>/context-init/telemetry/2026-10.jsonl
        let enabled_sink = JsonlTelemetrySink::new(&data_dir, true);
        assert!(enabled_sink.is_enabled());
        enabled_sink.record(&ev_op).unwrap();
        enabled_sink.record(&ev_fb).unwrap();

        let expected_jsonl = data_dir
            .join("context-init")
            .join("telemetry")
            .join("2026-10.jsonl");
        assert!(expected_jsonl.is_file());

        let recorded = enabled_sink.read_all_events().unwrap();
        assert_eq!(recorded.len(), 2);
        assert_eq!(recorded[0].schema_version, TELEMETRY_SCHEMA_VERSION);
        assert_eq!(recorded[0].project_id_hash.len(), 64);
        assert!(!recorded[0].project_id_hash.contains("github.com"));

        // 3. Report redacts notes by default (include_notes = false, O6)
        let redacted_report = render_telemetry_report(&recorded, false);
        assert!(redacted_report.starts_with("---\n"));
        assert!(redacted_report.contains("include_notes: false"));
        assert!(redacted_report.contains("feedback_notes_redacted: 1"));
        assert!(!redacted_report.contains("CONFIDENTIAL_INTERNAL_MODULE_NAME"));

        // 4. Report includes notes when explicitly requested (include_notes = true)
        let full_report = render_telemetry_report(&recorded, true);
        assert!(full_report.contains("include_notes: true"));
        assert!(full_report.contains("CONFIDENTIAL_INTERNAL_MODULE_NAME"));
    }

    #[test]
    fn edge_cases_empty_xdg_all_model_kinds_and_partial_model_override() {
        let tmp = TempDirGuard::new("edge_cfg");
        let home = tmp.path().join("home");
        fs::create_dir_all(&home).unwrap();

        // Empty/whitespace XDG_* variables must fall back to ~/.config, ~/.local/share, ~/.cache
        let mut empty_env = BTreeMap::new();
        empty_env.insert("XDG_CONFIG_HOME".to_string(), "   ".to_string());
        empty_env.insert("XDG_DATA_HOME".to_string(), "".to_string());
        empty_env.insert("XDG_CACHE_HOME".to_string(), "\t".to_string());

        let paths =
            StoragePaths::resolve_with_env(None, Some(&home), |k| empty_env.get(k).cloned());
        assert_eq!(
            paths.global_config_path,
            home.join(".config")
                .join("context-init")
                .join("config.toml")
        );
        assert_eq!(
            paths.data_dir,
            home.join(".local").join("share").join("context-init")
        );
        assert_eq!(paths.cache_dir, home.join(".cache").join("context-init"));

        // All 7 ModelKind variants + partial model override in project config
        let global_toml = r#"
[models.m_ollama]
kind = "ollama"
model = "qwen2.5:14b-ctx64k"
endpoint = "http://127.0.0.1:11434"
timeout_ms = 60000

[models.m_cmd]
kind = "command"
command = ["claude", "-p"]

[models.m_agent]
kind = "agent"

[models.m_oai]
kind = "openai_compat"
endpoint = "http://127.0.0.1:8000/v1"

[models.m_anth]
kind = "anthropic"

[models.m_gem]
kind = "gemini"

[models.m_samp]
kind = "sampling"
"#;

        let project_toml = r#"
[models.m_ollama]
model = "overridden-model"
max_input_tokens = 16000

[roles]
init = ["m_ollama", "m_cmd", "m_agent", "m_oai", "m_anth", "m_gem", "m_samp", "none"]
"#;

        let cfg = Config::from_layers(
            Some(global_toml),
            Some(project_toml),
            |_| None,
            &CliOverrides::default(),
        )
        .unwrap();
        cfg.validate().unwrap();

        let ollama_cfg = &cfg.models["m_ollama"];
        assert_eq!(ollama_cfg.kind, ModelKind::Ollama);
        assert_eq!(ollama_cfg.model.as_deref(), Some("overridden-model"));
        assert_eq!(
            ollama_cfg.endpoint.as_deref(),
            Some("http://127.0.0.1:11434")
        );
        assert_eq!(ollama_cfg.timeout_ms, Some(60000));
        assert_eq!(ollama_cfg.max_input_tokens, Some(16000));

        // Error paths on invalid CLI role overrides and invalid model without kind
        assert!(CliOverrides::parse_role_override("init").is_err());
        assert!(CliOverrides::parse_role_override("init= , ").is_err());
        assert!(CliOverrides::parse_role_override("unknown_role=local").is_err());

        let mut bad_cfg = Config::default();
        assert!(bad_cfg
            .apply_toml_str("[models.broken]\nmodel = \"no-kind\"\n")
            .is_err());
    }

    #[test]
    fn edge_cases_okf_errors_porcelain_renames_and_all_telemetry_signals() {
        // Malformed OKF cards must error cleanly
        assert!(OkfCard::parse("# No frontmatter").is_err());
        assert!(OkfCard::parse("---\ndirectory: src\n# unclosed").is_err());
        assert!(OkfCard::parse("---\nsource:\n  blob_sha: a\n  commit_sha: b\n---\n").is_err());
        assert!(OkfCard::parse(
            "---\ndirectory: src\nsource:\n  blob_sha: a\n  commit_sha: b\ngenerated_by:\n  role: init\n  model: none\nfreshness:\n  total_entries: not_a_number\n---\n"
        )
        .is_err());

        // Empty markdown abstract extraction
        assert_eq!(extract_l0_abstract(""), "");
        assert_eq!(extract_l0_abstract("# Only Heading\n"), "");

        // Porcelain -z with rename (R), copy (C), spaces, and Windows backslashes
        let raw_porcelain =
            b"M  src\\modified file.rs\0R  src/new_name.rs\0src\\old_name.rs\0?? untracked.txt\0";
        let entries = parse_porcelain_z(raw_porcelain);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].status_code(), "M ");
        assert_eq!(entries[0].path, "src/modified file.rs");
        assert_eq!(entries[0].orig_path, None);

        assert_eq!(entries[1].status_code(), "R ");
        assert_eq!(entries[1].path, "src/new_name.rs");
        assert_eq!(entries[1].orig_path.as_deref(), Some("src/old_name.rs"));

        assert_eq!(entries[2].status_code(), "??");
        assert_eq!(entries[2].path, "untracked.txt");

        // All 5 telemetry signal variants in report
        let events = vec![
            TelemetryEvent::with_timestamp(
                "proj-1",
                "2026-10-19",
                TelemetrySignal::Operational(OperationalSignal {
                    tool: "context_check".to_string(),
                    channel: "cli".to_string(),
                    harness: None,
                    latency_ms: 15,
                    success: false,
                    error_class: Some("DriftDetected".to_string()),
                    version: VERSION.to_string(),
                    os: "linux".to_string(),
                }),
            ),
            TelemetryEvent::with_timestamp(
                "proj-1",
                "2026-10-19",
                TelemetrySignal::Efficiency(EfficiencySignal {
                    tool: "context_pack".to_string(),
                    response_tokens: 900,
                    source_tokens: 4500,
                    truncated: true,
                    cursor_hops: 2,
                    stale: true,
                    empty_search: false,
                }),
            ),
            TelemetryEvent::with_timestamp(
                "proj-1",
                "2026-10-19",
                TelemetrySignal::Llm(LlmSignal {
                    alias: "local".to_string(),
                    kind: "ollama".to_string(),
                    role: "init".to_string(),
                    input_tokens: 1200,
                    output_tokens: 350,
                    fallback_used: true,
                    schema_errors: 1,
                    retries: 2,
                }),
            ),
            TelemetryEvent::with_timestamp(
                "proj-1",
                "2026-10-19",
                TelemetrySignal::SessionPattern(SessionPatternSignal {
                    call_sequence: vec![
                        "context_get_subsystems".to_string(),
                        "context_get_subsystem".to_string(),
                    ],
                    wandering_detected: true,
                }),
            ),
        ];

        let digest = render_telemetry_report(&events, false);
        assert!(digest.contains("total_events: 4"));
        assert!(digest.contains("DriftDetected"));
        assert!(digest.contains("Response tokens: 900"));
        assert!(digest.contains("Fallbacks triggered: 1"));
        assert!(digest.contains("Wandering detected: 1"));
    }

    #[test]
    fn regression_yaml_inline_comments_flow_maps_and_l0_abstract_boundaries() {
        // 1. Inline YAML comments, tabs, quoted '#', and flow-style section maps
        let card_md = concat!(
            "---   \n",
            "directory: \"src/core#v1\" # inline comment after quoted value\n",
            "source: { kind: git, blob_sha: 1234abcd, commit_sha: 5678ef01 } # flow map\n",
            "generated_by:\n",
            "\tcomponent: context-init # tab-indented\n",
            "\trole: update # role comment\n",
            "\tmodel: local\n",
            "freshness:\n",
            "  total_entries: 15 # integer with inline comment\n",
            "  sampled_entries: 10 # sampled\n",
            "  unsampled_entries: 5\n",
            "  pending_child_changes: 0\n",
            "---\n\n",
            "# Heading Without Intro Paragraph\n\n",
            "## Quick Navigation\n",
            "- `item_1`: should not be extracted as L0 abstract\n",
        );

        let card = OkfCard::parse(card_md).expect("parse card with comments and flow map");
        assert_eq!(card.frontmatter.directory, "src/core#v1");
        assert_eq!(card.frontmatter.source.kind, "git");
        assert_eq!(card.frontmatter.source.blob_sha, "1234abcd");
        assert_eq!(card.frontmatter.source.commit_sha, "5678ef01");
        assert_eq!(card.frontmatter.generated_by.role, "update");
        assert_eq!(card.frontmatter.freshness.total_entries, 15);
        assert_eq!(card.frontmatter.freshness.sampled_entries, 10);

        // H1 immediately followed by H2 + bullet list has no intro paragraph
        assert_eq!(card.l0_abstract(), "");
        assert_eq!(extract_l0_abstract(card_md), "");

        // Invalid opening delimiter `---invalid` must be rejected
        assert!(OkfCard::parse("---invalid\ndirectory: src\n---\n").is_err());
    }

    #[test]
    fn regression_git_staleness_empty_blob_detached_head_and_unborn_branch() {
        let backend = SystemGitBackend::new();
        let tmp = TempDirGuard::new("git_edge");
        let repo = tmp.path();

        let run = |args: &[&str]| {
            let out = Command::new("git")
                .arg("-C")
                .arg(repo)
                .args(args)
                .output()
                .expect("git command");
            assert!(
                out.status.success(),
                "git {:?} failed: {}",
                args,
                String::from_utf8_lossy(&out.stderr)
            );
        };

        run(&["init"]);
        run(&["config", "user.email", "ci@example.com"]);
        run(&["config", "user.name", "CI Tester"]);

        // 1. Unborn branch (before first commit): head_commit errors cleanly, file blob_sha works
        assert!(backend.head_commit(repo).is_err());
        let src_dir = repo.join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("lib.rs"), "pub fn init() {}\n").unwrap();
        let unborn_file_sha = backend
            .blob_sha(repo, Path::new("src/lib.rs"))
            .unwrap()
            .expect("hash-object works on unborn branch");
        assert!(!unborn_file_sha.is_empty());

        // 2. Commit and test empty recorded_blob_sha vs existing blob
        run(&["add", "."]);
        run(&["commit", "-m", "c1"]);
        let c1 = backend.head_commit(repo).unwrap();

        let card_empty_blob = OkfCard::new(
            CardFrontmatter {
                directory: "src".to_string(),
                source: CardSource {
                    kind: "git".to_string(),
                    blob_sha: String::new(),
                    commit_sha: c1.clone(),
                },
                generated_by: CardGeneratedBy::default(),
                freshness: CardFreshness::default(),
            },
            "# Src\n\nBody.\n",
        );

        let report_empty_blob = check_card_staleness(&backend, repo, &card_empty_blob).unwrap();
        assert!(
            report_empty_blob.stale,
            "Card with empty recorded_blob_sha must be marked stale when directory exists"
        );

        // 3. Detached HEAD state
        run(&["checkout", "--detach", &c1]);
        assert_eq!(backend.head_commit(repo).unwrap(), c1);
        let full_src_path = repo.join("src");
        let sha_rel = backend.blob_sha(repo, Path::new("src")).unwrap();
        let sha_abs = backend.blob_sha(repo, &full_src_path).unwrap();
        assert_eq!(sha_rel, sha_abs);
    }

    #[test]
    fn regression_scp_remote_at_sign_dot_segments_and_strict_toml_validation() {
        // SCP remote without '/' where repo name contains '@'
        assert_eq!(
            normalize_git_remote("git@myhost.local:pkg@2.git"),
            "myhost.local/pkg@2"
        );

        // Intermediate `/./` and trailing `/.` path normalization
        assert_eq!(normalize_path("src/./core/./lib.rs"), "src/core/lib.rs");
        assert_eq!(normalize_path("src/."), "src");

        // `timeout` alias in [models.<alias>] per D16
        let cfg = Config::from_layers(
            Some(
                r#"
[models.local]
kind = "ollama"
model = "qwen2.5:14b-ctx64k"
timeout = 45000
"#,
            ),
            None,
            |_| None,
            &CliOverrides::default(),
        )
        .expect("timeout alias must be accepted");
        assert_eq!(cfg.models["local"].timeout_ms, Some(45000));

        // Unknown TOML field must be rejected
        assert!(Config::from_layers(
            Some("max_reponse_tokens = 1234\n"),
            None,
            |_| None,
            &CliOverrides::default(),
        )
        .is_err());

        // ModelKind::Command without non-empty command must fail validate()
        let bad_cmd_cfg = Config::from_layers(
            Some(
                r#"
[models.bad_cli]
kind = "command"
command = []
"#,
            ),
            None,
            |_| None,
            &CliOverrides::default(),
        )
        .unwrap();
        assert!(bad_cmd_cfg.validate().is_err());
    }

    #[test]
    fn regression_f1_to_f5_audit_fixes() {
        // F1: Numeric namespace in SCP remote (no scheme) must NOT be stripped as a port
        assert_eq!(
            normalize_git_remote("git@github.com:12345/repo.git"),
            "github.com/12345/repo"
        );
        assert_eq!(
            normalize_git_remote("https://github.com/12345/repo.git"),
            "github.com/12345/repo"
        );
        // With scheme, numeric segment after ':' IS a port and must be stripped
        assert_eq!(
            normalize_git_remote("ssh://git@github.com:2222/12345/repo.git"),
            "github.com/12345/repo"
        );

        // F2: Empty home_dir argument in resolve_with_env must fall back to HOME / USERPROFILE
        let tmp = TempDirGuard::new("f2_f3");
        let env_home = tmp.path().join("env_home");
        fs::create_dir_all(&env_home).unwrap();
        let mut env_map = BTreeMap::new();
        env_map.insert("HOME".to_string(), env_home.to_string_lossy().to_string());
        let paths_empty_home = StoragePaths::resolve_with_env(None, Some(Path::new("   ")), |k| {
            env_map.get(k).cloned()
        });
        assert_eq!(
            paths_empty_home.global_config_path,
            env_home
                .join(".config")
                .join("context-init")
                .join("config.toml")
        );

        // F3: APPDATA fallback must NOT be populated when CONTEXT_INIT_CONFIG or XDG_CONFIG_HOME is set
        let appdata_dir = tmp.path().join("appdata");
        let appdata_cfg = appdata_dir.join("context-init").join("config.toml");
        fs::create_dir_all(appdata_cfg.parent().unwrap()).unwrap();
        fs::write(&appdata_cfg, "language = \"en\"\n").unwrap();

        let explicit_cfg = tmp.path().join("explicit_nonexistent_config.toml");
        let mut explicit_env = BTreeMap::new();
        explicit_env.insert(
            "APPDATA".to_string(),
            appdata_dir.to_string_lossy().to_string(),
        );
        explicit_env.insert(
            "CONTEXT_INIT_CONFIG".to_string(),
            explicit_cfg.to_string_lossy().to_string(),
        );
        let paths_explicit =
            StoragePaths::resolve_with_env(None, Some(&env_home), |k| explicit_env.get(k).cloned());
        assert_eq!(paths_explicit.fallback_global_config_path, None);
        assert_eq!(
            paths_explicit.effective_global_config_path(),
            explicit_cfg.as_path()
        );

        let xdg_dir = tmp.path().join("custom_xdg");
        let mut xdg_env = BTreeMap::new();
        xdg_env.insert(
            "APPDATA".to_string(),
            appdata_dir.to_string_lossy().to_string(),
        );
        xdg_env.insert(
            "XDG_CONFIG_HOME".to_string(),
            xdg_dir.to_string_lossy().to_string(),
        );
        let paths_xdg =
            StoragePaths::resolve_with_env(None, Some(&env_home), |k| xdg_env.get(k).cloned());
        assert_eq!(paths_xdg.fallback_global_config_path, None);
        assert_eq!(
            paths_xdg.effective_global_config_path(),
            xdg_dir.join("context-init").join("config.toml").as_path()
        );

        // F4: CardFrontmatter::to_yaml must quote values containing '#', ':', or edge spaces
        let card_f4 = OkfCard::new(
            CardFrontmatter {
                directory: "src/dir #1:sub".to_string(),
                source: CardSource {
                    kind: "git #v2".to_string(),
                    blob_sha: "abcd1234".to_string(),
                    commit_sha: "ef015678".to_string(),
                },
                generated_by: CardGeneratedBy {
                    component: "context-init:core".to_string(),
                    role: " init ".to_string(),
                    model: "local:14b #fast".to_string(),
                },
                freshness: CardFreshness {
                    total_entries: 4,
                    sampled_entries: 4,
                    unsampled_entries: 0,
                    pending_child_changes: 0,
                },
            },
            "# Module\n\nSummary text.\n",
        );
        let md_f4 = card_f4.to_markdown();
        let reparsed_f4 = OkfCard::parse(&md_f4).expect("roundtrip card with #, :, and spaces");
        assert_eq!(reparsed_f4.frontmatter, card_f4.frontmatter);

        // F5: check_frontmatter_staleness must mark card as stale when status_porcelain_z fails
        struct FailingStatusGitBackend;
        impl GitBackend for FailingStatusGitBackend {
            fn head_commit(&self, _repo_root: &Path) -> anyhow::Result<String> {
                Ok("ef015678".to_string())
            }
            fn remote_url(&self, _repo_root: &Path) -> anyhow::Result<Option<String>> {
                Ok(None)
            }
            fn blob_sha(
                &self,
                _repo_root: &Path,
                _relative_path: &Path,
            ) -> anyhow::Result<Option<String>> {
                Ok(Some("abcd1234".to_string()))
            }
            fn status_porcelain_z(&self, _repo_root: &Path) -> anyhow::Result<Vec<GitStatusEntry>> {
                anyhow::bail!("simulated git status failure")
            }
            fn diff_name_only(
                &self,
                _repo_root: &Path,
                _from: &str,
                _to: &str,
            ) -> anyhow::Result<Vec<String>> {
                Ok(Vec::new())
            }
            fn diff_stat(
                &self,
                _repo_root: &Path,
                _from: &str,
                _to: &str,
                _paths: &[String],
            ) -> anyhow::Result<String> {
                Ok(String::new())
            }
        }

        let report_f5 =
            check_card_staleness(&FailingStatusGitBackend, tmp.path(), &card_f4).unwrap();
        assert!(
            report_f5.stale,
            "Card must be marked stale when status_porcelain_z fails"
        );
        assert_eq!(report_f5.diff_stat, "working tree status check failed");
    }
}
