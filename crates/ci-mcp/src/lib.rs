//! Model Context Protocol (MCP) server, pagination, and tool handlers for `context-init` (D14, D19–D21, D29).

pub mod handlers;
pub mod pagination;
pub mod pipeline;
pub mod server;
pub mod tools;

pub use handlers::{ContextHandler, HandlerReply, ResolvedProject};
pub use pagination::{
    count_tokens, decode_cursor, encode_cursor, format_footer, paginate_content, CursorData,
    PaginatedOutput,
};
pub use server::{find_project_root_upward, run_mcp_server, ContextInitServer};
pub use tools::{
    FeedbackResult, L0SubsystemSummary, McpCheckParams, McpCheckResult, McpFeedbackParams,
    McpGetSubsystemParams, McpGetSubsystemsParams, McpInitParams, McpPackParams, McpScanParams,
    McpSearchParams, McpSubmitParams, McpTaskParams, McpUpdateParams, McpWorkspaceParams,
    SubmitResult,
};

/// Current crate version synchronized with the workspace package metadata.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns the crate identifier and version string, linked with `ci-core` and `ci-llm`.
pub fn crate_info() -> String {
    format!(
        "ci-mcp/{} (core={}, llm={})",
        VERSION,
        ci_core::VERSION,
        ci_llm::VERSION
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ci_core::storage::{
        L0Index, SubsystemFile, SubsystemInfo, CURRENT_L0_SCHEMA_VERSION, SUBSYSTEM_ID_PATTERN,
    };
    use ci_core::telemetry::{TelemetryEvent, TelemetryScope, TelemetrySignal, TelemetrySink};
    use ci_llm::{AgentWorkPackage, RoleName};
    use rmcp::handler::server::wrapper::Parameters;
    use serde_json::json;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};
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
            let path = std::env::temp_dir().join(format!("ci_mcp_{prefix}_{pid}_{nanos}_{seq}"));
            fs::create_dir_all(&path).expect("create temp test dir");
            // Keep central storage (`$XDG_DATA_HOME/context-init`) out of the real home.
            std::env::set_var("XDG_DATA_HOME", path.join(".xdg-data"));
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

    #[derive(Default)]
    struct MockTelemetrySink {
        events: Mutex<Vec<TelemetryEvent>>,
    }

    impl TelemetrySink for MockTelemetrySink {
        fn is_enabled(&self) -> bool {
            true
        }

        fn record(&self, event: &TelemetryEvent) -> anyhow::Result<()> {
            self.events.lock().unwrap().push(event.clone());
            Ok(())
        }
    }

    #[test]
    fn smoke_version_and_workspace_links() {
        assert_eq!(VERSION, "0.4.0");
        assert_eq!(VERSION, ci_core::VERSION);
        assert_eq!(VERSION, ci_llm::VERSION);
        assert!(crate_info().contains("ci-mcp/0.4.0"));
    }

    #[test]
    fn pagination_and_truncation_footer_contract() {
        // 1. Footer formatting
        let footer_trunc = format_footer(true, Some("cur_abc123"), false, 3950);
        assert_eq!(
            footer_trunc,
            "[truncated: true | next_cursor: \"cur_abc123\" | stale: false | tokens: 3950]"
        );

        let footer_trunc_stale = format_footer(true, Some("cur_xyz789"), true, 4000);
        assert_eq!(
            footer_trunc_stale,
            "[truncated: true | next_cursor: \"cur_xyz789\" | stale: true | tokens: 4000]"
        );

        let footer_clean = format_footer(false, None, true, 800);
        assert_eq!(
            footer_clean,
            "[truncated: false | stale: true | tokens: 800]"
        );

        // 2. Cursor serialization roundtrip
        let cur = CursorData {
            byte_offset: 1024,
            page: 2,
        };
        let encoded = encode_cursor(&cur);
        assert!(encoded.starts_with("cur_"));
        let decoded = decode_cursor(&encoded).expect("decode cursor");
        assert_eq!(decoded, cur);

        // 3. Multi-page pagination respecting token budget
        let lines: Vec<String> = (0..200)
            .map(|i| format!("pub fn handler_{i}() -> Result<()> {{\n    Ok(())\n}}"))
            .collect();
        let payload = lines.join("\n\n");

        let token_budget = 150;
        let mut cur_opt = None;
        let mut accumulated_body = String::new();
        let mut pages_count = 0;

        for _ in 0..50 {
            let page = paginate_content(&payload, cur_opt.as_deref(), token_budget, false);
            pages_count += 1;
            assert!(
                page.token_count <= token_budget,
                "Rendered page tokens {} must be <= budget {}",
                page.token_count,
                token_budget
            );
            accumulated_body.push_str(&page.body);

            if page.truncated {
                assert!(page.next_cursor.is_some());
                assert!(page
                    .rendered
                    .contains("[truncated: true | next_cursor: \"cur_"));
                cur_opt = page.next_cursor;
            } else {
                break;
            }
        }

        assert!(pages_count > 1, "Payload should span multiple pages");
        assert_eq!(accumulated_body, payload);
    }

    #[tokio::test]
    async fn cascade_project_root_resolution_order() {
        let tmp = TempDirGuard::new("cascade");
        let dir_a = tmp.path().join("project_a");
        let dir_b = tmp.path().join("project_b");
        let dir_c = tmp.path().join("project_c");
        fs::create_dir_all(&dir_a).unwrap();
        fs::create_dir_all(&dir_b).unwrap();
        fs::create_dir_all(&dir_c).unwrap();

        fs::write(dir_a.join(".context-init.toml"), "language = \"en\"\n").unwrap();
        fs::write(dir_b.join(".context-init.toml"), "language = \"de\"\n").unwrap();
        fs::write(dir_c.join(".context-init.toml"), "language = \"fr\"\n").unwrap();

        let server = ContextInitServer::new(Some(dir_c.clone()));

        // Step 1: Explicit argument `project` or `path` wins over roots and default_root
        server.add_root(dir_b.clone()).await;
        let r1 = server
            .resolve_project_root(Some(&dir_a.to_string_lossy()), None)
            .await;
        assert_eq!(r1, fs::canonicalize(&dir_a).unwrap());

        let r1_proj = server
            .resolve_project_root(None, Some(&dir_a.to_string_lossy()))
            .await;
        assert_eq!(r1_proj, fs::canonicalize(&dir_a).unwrap());

        // Step 1b: Upward search from a nested subdirectory resolves to project root
        let nested = dir_a.join("src").join("handlers");
        fs::create_dir_all(&nested).unwrap();
        let r1_up = server
            .resolve_project_root(Some(&nested.to_string_lossy()), None)
            .await;
        assert_eq!(r1_up, fs::canonicalize(&dir_a).unwrap());

        // Step 2: Registered MCP client root wins when argument is None
        let r2 = server.resolve_project_root(None, None).await;
        assert_eq!(r2, fs::canonicalize(&dir_b).unwrap());

        // Step 3: Default root wins when no roots are registered and no argument is given
        server.set_roots(Vec::new()).await;
        let r3 = server.resolve_project_root(None, None).await;
        assert_eq!(r3, fs::canonicalize(&dir_c).unwrap());
    }

    fn write_index(dir: &Path, summary: &str) {
        fs::create_dir_all(dir).unwrap();
        let index = L0Index {
            schema_version: CURRENT_L0_SCHEMA_VERSION,
            project: "demo".to_string(),
            subsystems: vec![SubsystemInfo {
                id: "core".to_string(),
                name: "Core Logic".to_string(),
                summary: summary.to_string(),
                l1_path: "subsystems/core.md".to_string(),
                tags: vec![],
                entrypoints: vec![],
                key_files: vec![],
                files: vec![],
                key_symbols: vec![],
                test_command: None,
                tokens: 0,
                parent: None,
                summary_source: ci_core::SummarySource::Heuristic,
                summary_model: None,
                needs_review: false,
            }],
            ..L0Index::default()
        };
        index.save_to_file(&dir.join("L0_index.json")).unwrap();
    }

    #[test]
    fn resolved_project_uses_configured_storage() {
        let tmp = TempDirGuard::new("kb_store");
        std::env::set_var("XDG_DATA_HOME", tmp.path().join("xdg"));
        let repo_root = tmp.path().join("my_repo");
        fs::create_dir_all(&repo_root).unwrap();
        fs::write(repo_root.join("Cargo.toml"), "[package]\nname = \"demo\"\n").unwrap();
        let canonical = fs::canonicalize(&repo_root).unwrap();

        let repo = ResolvedProject::load(repo_root.clone()).unwrap();
        assert_eq!(repo.storage, ci_core::StorageMode::Repo);
        assert_eq!(
            repo.knowledge_dir,
            canonical.join(".context-init/knowledge")
        );

        fs::write(
            repo_root.join(".context-init.toml"),
            "storage = \"central\"\n",
        )
        .unwrap();
        let central = ResolvedProject::load(repo_root).unwrap();
        assert_eq!(central.storage, ci_core::StorageMode::Central);
        assert!(
            central.knowledge_dir.starts_with(tmp.path().join("xdg"))
                || central
                    .knowledge_dir
                    .to_string_lossy()
                    .contains(".xdg-data")
        );
        write_index(&central.knowledge_dir, "Central storage subsystem");
        assert_eq!(
            central.load_l0_index().unwrap().subsystems[0].summary,
            "Central storage subsystem"
        );
    }

    #[test]
    fn storage_mismatch_hint_points_to_other_mode() {
        let tmp = TempDirGuard::new("kb_mismatch");
        std::env::set_var("XDG_DATA_HOME", tmp.path().join("xdg"));
        let repo_root = tmp.path().join("my_repo");
        fs::create_dir_all(&repo_root).unwrap();
        let project = ResolvedProject::load(repo_root).unwrap();
        assert_eq!(project.storage, ci_core::StorageMode::Repo);
        assert!(pipeline::storage_mismatch_hint(&project).is_none());

        write_index(
            &project.knowledge_dir_for(ci_core::StorageMode::Central),
            "elsewhere",
        );
        let hint = pipeline::storage_mismatch_hint(&project).expect("hint");
        assert!(hint.contains("migrate --to central"), "{hint}");

        write_index(&project.knowledge_dir, "here");
        assert!(pipeline::storage_mismatch_hint(&project).is_none());
    }

    /// Seeds an in-repo knowledge base (lazy fallback) with one `core` subsystem; returns its dir.
    fn seed_in_repo_knowledge(root: &Path) -> PathBuf {
        let kd = root.join(".context-init").join("knowledge");
        fs::create_dir_all(kd.join("subsystems")).unwrap();
        let index = L0Index {
            subsystems: vec![SubsystemInfo {
                id: "core".to_string(),
                name: "Core".to_string(),
                summary: "Core subsystem".to_string(),
                l1_path: "subsystems/core.md".to_string(),
                tags: vec![],
                entrypoints: vec![],
                key_files: vec![],
                files: vec![SubsystemFile {
                    path: "src/lib.rs".to_string(),
                    is_entrypoint: true,
                    lines: 10,
                }],
                key_symbols: vec![],
                test_command: None,
                tokens: 10,
                parent: None,
                summary_source: ci_core::SummarySource::Heuristic,
                summary_model: None,
                needs_review: false,
            }],
            ..L0Index::default()
        };
        index.save_to_file(&kd.join("L0_index.json")).unwrap();
        kd
    }

    async fn submit_with_package(
        server: &ContextInitServer,
        root: &Path,
        role: RoleName,
        payload: serde_json::Value,
    ) -> SubmitResult {
        let pkg = AgentWorkPackage {
            package_id: format!("pkg-test-{}-1", role.as_str()),
            role,
            facts: json!({}),
            instruction: "test".to_string(),
            json_schema: ci_llm::default_role_json_schema(role),
            alias: "test".to_string(),
            deep: false,
            agents_md: false,
            project_root: Some(fs::canonicalize(root).unwrap()),
        };
        server.register_work_package(pkg.clone()).await;
        let out = server
            .submit(Parameters(McpSubmitParams {
                package_id: pkg.package_id,
                submission: payload,
                path: Some(root.to_string_lossy().to_string()),
                project: None,
            }))
            .await
            .expect("submit tool call");
        serde_json::from_str(&out).expect("parse SubmitResult JSON")
    }

    #[allow(deprecated)]
    fn root(uri: &str) -> rmcp::model::Root {
        serde_json::from_value(json!({ "uri": uri })).unwrap()
    }

    #[test]
    #[allow(deprecated)]
    fn roots_to_paths_parses_file_uris() {
        #[cfg(not(windows))]
        {
            let paths = crate::server::roots_to_paths(&[
                root("file:///tmp/a%20b"),
                root("https://example.com/repo"),
                root("file:///srv/%D0%BF%D1%80%D0%BE%D0%B5%D0%BA%D1%82"),
            ]);
            assert_eq!(
                paths,
                vec![PathBuf::from("/tmp/a b"), PathBuf::from("/srv/проект")]
            );
        }
        #[cfg(windows)]
        assert_eq!(
            crate::server::roots_to_paths(&[root("file:///C:/x")]),
            vec![PathBuf::from("C:/x")]
        );
    }

    #[tokio::test]
    async fn resolution_uses_roots_after_refresh() {
        let tmp = TempDirGuard::new("roots_cascade");
        let other = TempDirGuard::new("roots_default");
        fs::write(tmp.path().join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        let server = ContextInitServer::new(Some(other.path().to_path_buf()));
        server.set_roots(vec![tmp.path().to_path_buf()]).await;
        let resolved = server.resolve_project_root(None, None).await;
        assert_eq!(resolved, fs::canonicalize(tmp.path()).unwrap());
        let explicit = server
            .resolve_project_root(Some(&other.path().to_string_lossy()), None)
            .await;
        assert_eq!(explicit, fs::canonicalize(other.path()).unwrap());
    }

    /// A small Rust project with three modules.
    fn write_fixture(root: &Path) {
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("src/main.rs"),
            "mod api;\nmod engine;\nmod store;\nfn main() {}\n",
        )
        .unwrap();
        for m in ["api", "engine", "store"] {
            fs::create_dir_all(root.join("src").join(m)).unwrap();
            fs::write(
                root.join("src").join(m).join("mod.rs"),
                format!("pub struct {m}Thing;\npub fn run_{m}() -> u32 {{ 1 }}\n"),
            )
            .unwrap();
        }
    }

    fn init_params(root: &Path) -> McpInitParams {
        McpInitParams {
            path: Some(root.to_string_lossy().to_string()),
            force: Some(true),
            with_agents_md: Some(false),
            ..McpInitParams::default()
        }
    }

    const AGENT_CONFIG: &str = "[models.h]\nkind = \"agent\"\n\n[roles]\ninit = [\"h\"]\n";

    #[tokio::test]
    async fn stale_binary_footer_appears_when_mtime_changes() {
        let tmp = TempDirGuard::new("stale_footer");
        write_fixture(tmp.path());
        let mut server = ContextInitServer::new(Some(tmp.path().to_path_buf()));
        server
            .init(Parameters(init_params(tmp.path())))
            .await
            .expect("init");
        let footer = "⚠ context-init обновлён на диске — перезапустите MCP-сервер";
        let params = || {
            Parameters(McpCheckParams {
                path: Some(tmp.path().to_string_lossy().to_string()),
                ..McpCheckParams::default()
            })
        };

        assert_eq!(server.stale_binary_footer(), None);
        let fresh = server.check_drift(params()).await.unwrap();
        assert!(!fresh.contains(footer), "{fresh}");

        server.exe_mtime_at_start = Some(std::time::UNIX_EPOCH);
        assert_eq!(server.stale_binary_footer(), Some(footer));
        let stale = server.check_drift(params()).await.unwrap();
        assert!(stale.ends_with(&format!("\n{footer}")), "{stale}");
    }

    #[test]
    fn stale_binary_footer_follows_real_file_replacement() {
        let tmp = TempDirGuard::new("stale_real");
        let exe = tmp.path().join("fake-exe");
        fs::write(&exe, "v1").unwrap();
        let server_for = |exe: &Path| {
            let mut s = ContextInitServer::new(None);
            s.exe_path = Some(exe.to_path_buf());
            s.exe_mtime_at_start = fs::metadata(exe).and_then(|m| m.modified()).ok();
            s
        };
        let server = server_for(&exe);
        assert_eq!(server.stale_binary_footer(), None, "untouched");

        // Atomic install: write a sibling, give it a different mtime, rename over the target.
        let new = tmp.path().join(".fake-exe.new");
        fs::write(&new, "v2").unwrap();
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
        fs::File::options()
            .write(true)
            .open(&new)
            .unwrap()
            .set_modified(later)
            .unwrap();
        fs::rename(&new, &exe).unwrap();
        assert!(server.stale_binary_footer().is_some(), "replaced");

        // Removed, then recreated with a different mtime.
        let server = server_for(&exe);
        fs::remove_file(&exe).unwrap();
        assert!(server.stale_binary_footer().is_some(), "deleted");
        fs::write(&exe, "v3").unwrap();
        fs::File::options()
            .write(true)
            .open(&exe)
            .unwrap()
            .set_modified(later + std::time::Duration::from_secs(60))
            .unwrap();
        assert!(server.stale_binary_footer().is_some(), "recreated");
    }

    #[tokio::test]
    async fn context_init_on_fixture_produces_real_subsystems() {
        let tmp = TempDirGuard::new("init_real");
        write_fixture(tmp.path());
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));
        server
            .init(Parameters(init_params(tmp.path())))
            .await
            .expect("init");

        let resolved = server
            .resolve_context(Some(&tmp.path().to_string_lossy()), None)
            .await;
        assert_eq!(resolved.storage, ci_core::StorageMode::Repo);
        assert!(resolved.knowledge_dir.join("L0_index.json").is_file());
        let index = resolved.load_l0_index().unwrap();
        assert!(index.subsystems.len() > 1, "{:?}", index.subsystems);
        assert!(!index
            .subsystems
            .iter()
            .any(|s| s.summary.starts_with("Core logic subsystem for project")));
        assert!(!tmp.path().join(".agents").exists());
    }

    #[tokio::test]
    async fn context_init_with_agent_model_registers_package() {
        let tmp = TempDirGuard::new("init_agent");
        write_fixture(tmp.path());
        fs::write(tmp.path().join(".context-init.toml"), AGENT_CONFIG).unwrap();
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));

        let reply = server
            .init(Parameters(init_params(tmp.path())))
            .await
            .expect("init");
        let package_id = reply
            .lines()
            .find_map(|l| l.strip_prefix("package_id: "))
            .unwrap_or_else(|| panic!("no package_id line in: {reply}"))
            .trim()
            .to_string();

        let resolved = server
            .resolve_context(Some(&tmp.path().to_string_lossy()), None)
            .await;
        let first_id = resolved.load_l0_index().unwrap().subsystems[0].id.clone();
        let out = server
            .submit(Parameters(McpSubmitParams {
                package_id,
                submission: json!({"subsystems": [{
                    "id": first_id, "name": "Better", "summary": "Agent summary", "tags": ["x"]
                }]}),
                path: Some(tmp.path().to_string_lossy().to_string()),
                project: None,
            }))
            .await
            .unwrap();
        let out: SubmitResult = serde_json::from_str(&out).unwrap();
        assert!(out.valid, "{:?}", out.errors);
    }

    /// Keeps the host's global config (and its telemetry setting) out of the test.
    fn isolate_config_dirs(tmp: &TempDirGuard) {
        std::env::set_var("XDG_CONFIG_HOME", tmp.path().join(".xdg-config"));
        std::env::set_var("HOME", tmp.path().join(".home"));
    }

    /// Runs `context_init` with an agent model on the fixture; returns the package id.
    async fn init_with_agent_package(server: &ContextInitServer, root: &Path) -> String {
        fs::write(root.join(".context-init.toml"), AGENT_CONFIG).unwrap();
        let reply = server
            .init(Parameters(init_params(root)))
            .await
            .expect("init");
        reply
            .lines()
            .find_map(|l| l.strip_prefix("package_id: "))
            .unwrap_or_else(|| panic!("no package_id line in: {reply}"))
            .trim()
            .to_string()
    }

    async fn submit_payload(
        server: &ContextInitServer,
        root: &Path,
        package_id: &str,
        submission: serde_json::Value,
    ) -> SubmitResult {
        let out = server
            .submit(Parameters(McpSubmitParams {
                package_id: package_id.to_string(),
                submission,
                path: Some(root.to_string_lossy().to_string()),
                project: None,
            }))
            .await
            .expect("submit tool call");
        serde_json::from_str(&out).expect("parse SubmitResult JSON")
    }

    #[tokio::test]
    async fn submit_updates_l1_card_and_l0_md() {
        let tmp = TempDirGuard::new("submit_card");
        isolate_config_dirs(&tmp);
        write_fixture(tmp.path());
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));
        let package_id = init_with_agent_package(&server, tmp.path()).await;
        let resolved = server
            .resolve_context(Some(&tmp.path().to_string_lossy()), None)
            .await;
        let target = resolved.load_l0_index().unwrap().subsystems[0].clone();

        let out = submit_payload(
            &server,
            tmp.path(),
            &package_id,
            json!({"subsystems": [{
                "id": target.id, "name": "Agent Name",
                "summary": "Agent-written summary of the subsystem", "tags": ["agent"]
            }]}),
        )
        .await;
        assert!(out.valid, "{:?}", out.errors);

        let card = fs::read_to_string(resolved.knowledge_dir.join(&target.l1_path)).unwrap();
        assert!(
            card.contains("Agent-written summary of the subsystem"),
            "{card}"
        );
        let l0_md = fs::read_to_string(resolved.knowledge_dir.join("L0_index.md")).unwrap();
        assert!(
            l0_md.contains("Agent-written summary of the subsystem"),
            "{l0_md}"
        );
        let index = resolved.load_l0_index().unwrap();
        let sub = index.subsystems.iter().find(|s| s.id == target.id).unwrap();
        assert_eq!(sub.summary_source, ci_core::SummarySource::Agent);
        assert_eq!(sub.summary_model.as_deref(), Some("h"));

        let report = pipeline::run_check(&resolved).unwrap();
        assert!(!report.has_drift, "{report:?}");
    }

    #[tokio::test]
    async fn submit_with_unknown_id_is_rejected_atomically() {
        let tmp = TempDirGuard::new("submit_atomic");
        isolate_config_dirs(&tmp);
        write_fixture(tmp.path());
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));
        let package_id = init_with_agent_package(&server, tmp.path()).await;
        let resolved = server
            .resolve_context(Some(&tmp.path().to_string_lossy()), None)
            .await;
        let kd = resolved.knowledge_dir.clone();
        let target = resolved.load_l0_index().unwrap().subsystems[0].clone();
        let index_before = fs::read(kd.join("L0_index.json")).unwrap();
        let md_before = fs::read(kd.join("L0_index.md")).unwrap();
        let card_before = fs::read(kd.join(&target.l1_path)).unwrap();

        let out = submit_payload(
            &server,
            tmp.path(),
            &package_id,
            json!({"subsystems": [
                {"id": target.id, "name": "Valid", "summary": "Valid summary", "tags": []},
                {"id": "ghost", "name": "Ghost", "summary": "Ghost summary", "tags": []}
            ]}),
        )
        .await;
        assert!(!out.valid);
        assert!(
            out.errors.iter().any(|e| e.contains("ghost")),
            "{:?}",
            out.errors
        );
        assert_eq!(fs::read(kd.join("L0_index.json")).unwrap(), index_before);
        assert_eq!(fs::read(kd.join("L0_index.md")).unwrap(), md_before);
        assert_eq!(fs::read(kd.join(&target.l1_path)).unwrap(), card_before);
        assert!(!kd.join("subsystems/ghost.md").exists());
    }

    #[tokio::test]
    async fn submit_rejects_single_card_body_form() {
        let tmp = TempDirGuard::new("submit_body_form");
        isolate_config_dirs(&tmp);
        write_fixture(tmp.path());
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));
        server
            .init(Parameters(init_params(tmp.path())))
            .await
            .expect("init");
        let resolved = server
            .resolve_context(Some(&tmp.path().to_string_lossy()), None)
            .await;
        let kd = resolved.knowledge_dir.clone();
        let target = resolved.load_l0_index().unwrap().subsystems[0].clone();
        let index_before = fs::read(kd.join("L0_index.json")).unwrap();
        let card_before = fs::read(kd.join(&target.l1_path)).unwrap();

        // The single-card form matches no role schema any more and must be refused.
        let out = submit_with_package(
            &server,
            tmp.path(),
            RoleName::Update,
            json!({"id": target.id, "summary": "s", "body": "# Replaced card"}),
        )
        .await;
        assert!(!out.valid, "{out:?}");
        assert_eq!(fs::read(kd.join("L0_index.json")).unwrap(), index_before);
        assert_eq!(fs::read(kd.join(&target.l1_path)).unwrap(), card_before);
    }

    #[tokio::test]
    async fn llm_enrichment_sets_llm_source() {
        let tmp = TempDirGuard::new("llm_source");
        isolate_config_dirs(&tmp);
        write_fixture(tmp.path());
        let root = fs::canonicalize(tmp.path()).unwrap();
        let analysis = ci_core::analysis::detector::analyze_project_with(
            &root,
            &ci_core::analysis::detector::AnalyzeOptions {
                deep: false,
                with_rtk: false,
                split_threshold_tokens: ci_core::config::Config::default().split_threshold_tokens,
                scope: Default::default(),
            },
        )
        .unwrap();
        let id = ci_core::analysis::subsystems::build_deterministic(&analysis)[0]
            .id
            .clone();
        let reply = tmp.path().join("llm_reply.json");
        fs::write(
            &reply,
            json!({"subsystems": [{
                "id": id, "name": "LLM Name", "summary": "LLM-written summary", "tags": ["llm"]
            }]})
            .to_string(),
        )
        .unwrap();
        fs::write(
            tmp.path().join(".context-init.toml"),
            format!(
                "[models.mock]\nkind = \"command\"\ncommand = [\"cat\", {:?}]\n\n[roles]\ninit = [\"mock\"]\n",
                reply.to_string_lossy()
            ),
        )
        .unwrap();
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));
        server
            .init(Parameters(init_params(tmp.path())))
            .await
            .expect("init");

        let resolved = server
            .resolve_context(Some(&tmp.path().to_string_lossy()), None)
            .await;
        let raw: serde_json::Value = serde_json::from_slice(
            &fs::read(resolved.knowledge_dir.join("L0_index.json")).unwrap(),
        )
        .unwrap();
        let entry = raw["subsystems"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["id"] == id.as_str())
            .unwrap_or_else(|| panic!("{id} missing in {raw}"));
        assert_eq!(entry["summary"], "LLM-written summary", "{entry}");
        assert_eq!(entry["summary_source"], "llm", "{entry}");
        assert_eq!(entry["summary_model"], "mock", "{entry}");
        let sub = resolved
            .load_l0_index()
            .unwrap()
            .subsystems
            .into_iter()
            .find(|s| s.id == id)
            .unwrap();
        let card = fs::read_to_string(resolved.knowledge_dir.join(&sub.l1_path)).unwrap();
        assert!(card.contains("LLM-written summary"), "{card}");
    }

    /// Writes a project config whose `role` chain is a `command` model answering `reply`.
    fn write_command_model_config(root: &Path, role: &str, reply: &serde_json::Value) {
        let reply_path = root.join("llm_reply.json");
        fs::write(&reply_path, reply.to_string()).unwrap();
        fs::write(
            root.join(".context-init.toml"),
            format!(
                "[models.mock]\nkind = \"command\"\ncommand = [\"cat\", {:?}]\n\n[roles]\n{role} = [\"mock\"]\n",
                reply_path.to_string_lossy()
            ),
        )
        .unwrap();
    }

    #[tokio::test]
    async fn llm_update_keeps_manual_and_agent_summaries() {
        let tmp = TempDirGuard::new("llm_keeps_manual");
        isolate_config_dirs(&tmp);
        write_fixture(tmp.path());
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));
        server
            .init(Parameters(init_params(tmp.path())))
            .await
            .expect("init");
        let resolved = server
            .resolve_context(Some(&tmp.path().to_string_lossy()), None)
            .await;
        let index_path = resolved.knowledge_dir.join("L0_index.json");
        let mut index = L0Index::load_from_file(&index_path).unwrap();
        assert!(index.subsystems.len() >= 3, "{:?}", index.subsystems);
        let ids: Vec<String> = index.subsystems.iter().map(|s| s.id.clone()).collect();
        index.subsystems[0].summary = "Hand-written summary".to_string();
        index.subsystems[0].summary_source = ci_core::SummarySource::Manual;
        index.subsystems[1].summary = "Agent-written summary".to_string();
        index.subsystems[1].summary_source = ci_core::SummarySource::Agent;
        index.subsystems[1].summary_model = Some("h".to_string());
        index.save_to_file(&index_path).unwrap();

        let items: Vec<serde_json::Value> = ids
            .iter()
            .map(|id| {
                json!({
                    "id": id, "name": "LLM", "summary": format!("LLM summary of {id}"),
                    "tags": ["llm"]
                })
            })
            .collect();
        let reply = json!({ "subsystems": items });
        write_command_model_config(tmp.path(), "update", &reply);
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));
        let reply = server
            .update(Parameters(McpUpdateParams {
                path: Some(tmp.path().to_string_lossy().to_string()),
                ..McpUpdateParams::default()
            }))
            .await
            .expect("update");
        assert!(reply.contains("LLM enrichment (mock) updated 1"), "{reply}");

        let after = L0Index::load_from_file(&index_path).unwrap();
        let get = |id: &str| after.subsystems.iter().find(|s| s.id == id).unwrap();
        let manual = get(&ids[0]);
        assert_eq!(manual.summary, "Hand-written summary");
        assert_eq!(manual.summary_source, ci_core::SummarySource::Manual);
        let agent = get(&ids[1]);
        assert_eq!(agent.summary, "Agent-written summary");
        assert_eq!(agent.summary_source, ci_core::SummarySource::Agent);
        assert_eq!(agent.summary_model.as_deref(), Some("h"));
        let heuristic = get(&ids[2]);
        assert_eq!(heuristic.summary, format!("LLM summary of {}", ids[2]));
        assert_eq!(heuristic.summary_source, ci_core::SummarySource::Llm);
        assert_eq!(heuristic.summary_model.as_deref(), Some("mock"));
    }

    const AGENT_UPDATE_CONFIG: &str =
        "[models.h]\nkind = \"agent\"\n\n[roles]\ninit = [\"h\"]\nupdate = [\"h\"]\n";

    /// Inits the fixture with an agent model and submits an agent summary for every subsystem.
    async fn init_with_all_agent_summaries(server: &ContextInitServer, root: &Path) {
        let package_id = init_with_agent_package(server, root).await;
        fs::write(root.join(".context-init.toml"), AGENT_UPDATE_CONFIG).unwrap();
        let resolved = server
            .resolve_context(Some(&root.to_string_lossy()), None)
            .await;
        let items: Vec<serde_json::Value> = resolved
            .load_l0_index()
            .unwrap()
            .subsystems
            .iter()
            .map(|s| json!({"id": s.id, "name": s.name, "summary": format!("Agent {}", s.id), "tags": []}))
            .collect();
        let out = submit_payload(server, root, &package_id, json!({ "subsystems": items })).await;
        assert!(out.valid, "{:?}", out.errors);
    }

    async fn run_context_update(server: &ContextInitServer, root: &Path) -> String {
        server
            .update(Parameters(McpUpdateParams {
                path: Some(root.to_string_lossy().to_string()),
                ..McpUpdateParams::default()
            }))
            .await
            .expect("update")
    }

    #[tokio::test]
    async fn update_package_lists_only_changed() {
        let tmp = TempDirGuard::new("update_only_changed");
        isolate_config_dirs(&tmp);
        write_fixture(tmp.path());
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));
        init_with_all_agent_summaries(&server, tmp.path()).await;
        let resolved = server
            .resolve_context(Some(&tmp.path().to_string_lossy()), None)
            .await;
        let total = resolved.load_l0_index().unwrap().subsystems.len();
        assert!(total >= 3, "{total}");

        fs::write(tmp.path().join("src/api/extra.rs"), "pub fn extra() {}\n").unwrap();
        let reply = run_context_update(&server, tmp.path()).await;
        assert!(reply.contains("package_id: "), "{reply}");
        let facts = reply
            .lines()
            .find_map(|l| l.strip_prefix("facts: "))
            .unwrap_or_else(|| panic!("no facts line in: {reply}"));
        assert_eq!(facts.matches("- id `").count(), 1, "{facts}");
        assert!(facts.contains("src/api/extra.rs"), "{facts}");
    }

    #[tokio::test]
    async fn update_without_changes_creates_no_package() {
        let tmp = TempDirGuard::new("update_no_changes");
        isolate_config_dirs(&tmp);
        write_fixture(tmp.path());
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));
        init_with_all_agent_summaries(&server, tmp.path()).await;
        let reply = run_context_update(&server, tmp.path()).await;
        assert!(
            reply
                .lines()
                .any(|l| l == "Обогащение не требуется: изменений нет."),
            "{reply}"
        );
        assert!(!reply.contains("package_id"), "{reply}");
        assert!(server.work_packages.read().await.is_empty());
    }

    #[tokio::test]
    async fn submit_refreshes_agents_md_written_by_init() {
        let tmp = TempDirGuard::new("submit_agents_md");
        isolate_config_dirs(&tmp);
        write_fixture(tmp.path());
        fs::write(tmp.path().join(".context-init.toml"), AGENT_CONFIG).unwrap();
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));
        // Default `context_init`: AGENTS.md is written while `touch_agents_md` stays false.
        let reply = server
            .init(Parameters(McpInitParams {
                path: Some(tmp.path().to_string_lossy().to_string()),
                force: Some(true),
                ..McpInitParams::default()
            }))
            .await
            .expect("init");
        let package_id = reply
            .lines()
            .find_map(|l| l.strip_prefix("package_id: "))
            .unwrap_or_else(|| panic!("no package_id line in: {reply}"))
            .trim()
            .to_string();
        let agents_md = tmp.path().join("AGENTS.md");
        assert!(agents_md.is_file());
        let resolved = server
            .resolve_context(Some(&tmp.path().to_string_lossy()), None)
            .await;
        assert!(!resolved.config.touch_agents_md);
        let id = resolved.load_l0_index().unwrap().subsystems[0].id.clone();

        let out = submit_payload(
            &server,
            tmp.path(),
            &package_id,
            json!({"subsystems": [{
                "id": id, "name": "N", "summary": "Summary refreshed into AGENTS", "tags": []
            }]}),
        )
        .await;
        assert!(out.valid, "{:?}", out.errors);
        let text = fs::read_to_string(&agents_md).unwrap();
        assert!(text.contains("Summary refreshed into AGENTS"), "{text}");
    }

    #[tokio::test]
    async fn submit_package_is_single_use() {
        let tmp = TempDirGuard::new("submit_single_use");
        isolate_config_dirs(&tmp);
        write_fixture(tmp.path());
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));
        let package_id = init_with_agent_package(&server, tmp.path()).await;
        let resolved = server
            .resolve_context(Some(&tmp.path().to_string_lossy()), None)
            .await;
        let index_path = resolved.knowledge_dir.join("L0_index.json");
        let id = resolved.load_l0_index().unwrap().subsystems[0].id.clone();
        let payload = |summary: &str| json!({"subsystems": [{"id": id, "name": "N", "summary": summary, "tags": []}]});

        // A rejected submission keeps the package for a retry.
        let bad = submit_payload(
            &server,
            tmp.path(),
            &package_id,
            json!({"subsystems": [{"id": "ghost", "name": "G", "summary": "g", "tags": []}]}),
        )
        .await;
        assert!(!bad.valid);
        let first = submit_payload(&server, tmp.path(), &package_id, payload("First")).await;
        assert!(first.valid, "{:?}", first.errors);
        let index_after_first = fs::read(&index_path).unwrap();

        let second = submit_payload(&server, tmp.path(), &package_id, payload("Second")).await;
        assert!(!second.valid);
        assert!(
            second
                .errors
                .iter()
                .any(|e| e.contains("Unknown package_id")),
            "{:?}",
            second.errors
        );
        assert_eq!(fs::read(&index_path).unwrap(), index_after_first);
    }

    #[tokio::test]
    async fn submit_rejects_package_of_another_project() {
        let tmp = TempDirGuard::new("submit_cross_project");
        isolate_config_dirs(&tmp);
        let (a, b) = (tmp.path().join("a"), tmp.path().join("b"));
        for root in [&a, &b] {
            fs::create_dir_all(root).unwrap();
            write_fixture(root);
        }
        let server = ContextInitServer::new(Some(a.clone()));
        let package_id = init_with_agent_package(&server, &a).await;
        server
            .init(Parameters(init_params(&b)))
            .await
            .expect("init b");
        let b_resolved = server
            .resolve_context(Some(&b.to_string_lossy()), None)
            .await;
        let b_index = b_resolved.knowledge_dir.join("L0_index.json");
        let b_before = fs::read(&b_index).unwrap();
        let id = b_resolved.load_l0_index().unwrap().subsystems[0].id.clone();

        let out = submit_payload(
            &server,
            &b,
            &package_id,
            json!({"subsystems": [{"id": id, "name": "N", "summary": "Cross", "tags": []}]}),
        )
        .await;
        assert!(!out.valid);
        assert!(
            out.errors.iter().any(|e| e.contains("was issued for")),
            "{:?}",
            out.errors
        );
        assert_eq!(fs::read(&b_index).unwrap(), b_before);
    }

    #[tokio::test]
    async fn check_reports_new_nested_module_as_untracked() {
        let tmp = TempDirGuard::new("check_untracked");
        write_fixture(tmp.path());
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));
        server
            .init(Parameters(init_params(tmp.path())))
            .await
            .expect("init");

        fs::create_dir_all(tmp.path().join("src/extra")).unwrap();
        fs::write(tmp.path().join("src/extra/mod.rs"), "pub fn extra() {}\n").unwrap();
        let out = server
            .check_drift(Parameters(McpCheckParams {
                path: Some(tmp.path().to_string_lossy().to_string()),
                ..McpCheckParams::default()
            }))
            .await
            .expect("check");
        let json_part = out.split("\n\n[").next().unwrap();
        let result: serde_json::Value = serde_json::from_str(json_part).unwrap();
        assert_eq!(result["has_drift"], true, "{out}");
        assert!(
            result["untracked_in_knowledge"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v.as_str().unwrap().contains("extra")),
            "{out}"
        );
    }

    fn git(root: &Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[tokio::test]
    async fn mcp_get_subsystem_reports_stale_diff_stat_and_auto_refreshes() {
        let tmp = TempDirGuard::new("stale_refresh");
        write_fixture(tmp.path());
        git(tmp.path(), &["init", "-q"]);
        git(tmp.path(), &["add", "-A"]);
        git(tmp.path(), &["commit", "-q", "-m", "base"]);
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));
        let path = Some(tmp.path().to_string_lossy().to_string());
        server
            .init(Parameters(init_params(tmp.path())))
            .await
            .expect("init");
        git(tmp.path(), &["add", "-A"]);
        git(tmp.path(), &["commit", "-q", "-m", "kb"]);
        async fn get(
            server: &ContextInitServer,
            path: &Option<String>,
        ) -> Result<String, rmcp::ErrorData> {
            server
                .get_subsystem(Parameters(McpGetSubsystemParams {
                    subsystem_id: "src_api".to_string(),
                    path: path.clone(),
                    project: None,
                    cursor: None,
                }))
                .await
        }

        let fresh = get(&server, &path).await.expect("fresh card");
        assert!(
            fresh.contains("stale: false") || !fresh.contains("stale: true"),
            "{fresh}"
        );

        fs::write(
            tmp.path().join("src/api/mod.rs"),
            "pub struct Thing;\npub fn run_api() -> u32 {\n    2\n}\n",
        )
        .unwrap();
        git(tmp.path(), &["commit", "-q", "-am", "change api"]);

        let stale = get(&server, &path).await.expect("stale card");
        assert!(stale.contains("stale: true"), "{stale}");
        assert!(stale.contains("## Git Staleness (diff --stat)"), "{stale}");
        assert!(stale.contains("src/api/mod.rs"), "{stale}");

        fs::write(
            tmp.path().join(".context-init.toml"),
            "auto_refresh = true\n",
        )
        .unwrap();
        let refreshed = get(&server, &path).await.expect("refreshed card");
        assert!(!refreshed.contains("stale: true"), "{refreshed}");
        assert!(!refreshed.contains("Git Staleness"), "{refreshed}");
        let again = get(&server, &path).await.expect("card after refresh");
        assert!(!again.contains("stale: true"), "{again}");
    }

    #[tokio::test]
    async fn detector_maps_configuration_subsystems_and_lazy_l2_object_cards() {
        let tmp = TempDirGuard::new("mdo");
        let write = |rel: &str, text: &str| {
            let p = tmp.path().join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, text).unwrap();
        };
        write(
            "src/Subsystems/Billing/Billing.mdo",
            "<mdclass:Subsystem xmlns:mdclass=\"urn:example:mdclass\"><name>Billing</name><content>Catalog.Items</content></mdclass:Subsystem>",
        );
        write(
            "src/Catalogs/Items/Items.mdo",
            "<mdclass:Catalog xmlns:mdclass=\"urn:example:mdclass\"><name>Items</name><attributes><name>Price</name></attributes><tabularSections><name>Prices</name><attributes><name>Period</name></attributes></tabularSections></mdclass:Catalog>",
        );
        write(
            "src/Catalogs/Items/ObjectModule.bsl",
            "Procedure Recalculate(Val Mode) Export\n    Total = 1;\nEndProcedure\n\nProcedure Hidden()\nEndProcedure\n",
        );
        write(
            "src/Catalogs/Orphans/Orphans.mdo",
            "<mdclass:Catalog xmlns:mdclass=\"urn:example:mdclass\"><name>Orphans</name></mdclass:Catalog>",
        );
        write(
            "src/CommonModules/Common/Module.bsl",
            "Function Version() Export\n    Return 1;\nEndFunction\n",
        );
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));
        let path = Some(tmp.path().to_string_lossy().to_string());
        server
            .init(Parameters(init_params(tmp.path())))
            .await
            .expect("init");
        let project = server.resolve_context(path.as_deref(), None).await;
        let ids: Vec<String> = project
            .load_l0_index()
            .unwrap()
            .subsystems
            .iter()
            .map(|s| s.id.clone())
            .collect();
        assert!(ids.contains(&"subsystem_billing".to_string()), "{ids:?}");
        assert!(ids.contains(&"metadata_catalog".to_string()), "{ids:?}");
        assert!(
            !ids.iter()
                .any(|i| i == "src_subsystems" || i == "src_catalogs"),
            "{ids:?}"
        );

        let l2 = server
            .get_subsystem(Parameters(McpGetSubsystemParams {
                subsystem_id: "catalog_items".to_string(),
                path: path.clone(),
                project: None,
                cursor: None,
            }))
            .await
            .expect("lazy L2 card");
        for needle in ["Price", "Prices", "Period", "Recalculate(Val Mode) Export"] {
            assert!(l2.contains(needle), "{needle} missing in {l2}");
        }
        assert!(!l2.contains("Hidden"), "{l2}");
    }

    #[tokio::test]
    async fn mcp_project_parameter_resolves_child_repo_by_name() {
        let tmp = TempDirGuard::new("ws_project");
        for name in ["billing-service", "gateway-service"] {
            let repo = tmp.path().join(name);
            fs::create_dir_all(repo.join("src")).unwrap();
            git(&repo, &["init", "-q"]);
            fs::write(repo.join("src/lib.rs"), "pub struct OrderDto;\n").unwrap();
        }
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));
        server.add_root(tmp.path().to_path_buf()).await;
        let root = server
            .resolve_project_root(None, Some("billing-service"))
            .await;
        assert_eq!(
            root,
            fs::canonicalize(tmp.path().join("billing-service")).unwrap()
        );

        let reply = server
            .get_subsystems(Parameters(McpGetSubsystemsParams {
                project: Some("billing-service".to_string()),
                ..McpGetSubsystemsParams::default()
            }))
            .await;
        // The child repo has no knowledge base yet: the error must name the child, not the workspace.
        let err = format!("{:?}", reply.unwrap_err());
        assert!(err.contains("billing-service"), "{err}");

        let map = server
            .workspace(Parameters(McpWorkspaceParams {
                path: Some(tmp.path().to_string_lossy().to_string()),
                ..McpWorkspaceParams::default()
            }))
            .await
            .expect("workspace");
        assert!(
            map.contains("billing-service") && map.contains("gateway-service"),
            "{map}"
        );
        assert!(map.contains("OrderDto"), "{map}");
    }

    #[tokio::test]
    async fn workspace_explicit_paths_reports_kb_status_and_disambiguates_names() {
        let tmp = TempDirGuard::new("ws_paths");
        let dir_a = tmp.path().join("dir_a/service");
        let dir_b = tmp.path().join("dir_b/service");
        let missing = tmp.path().join("missing/service");

        fs::create_dir_all(dir_a.join("src")).unwrap();
        fs::write(
            dir_a.join("Cargo.toml"),
            "[package]\nname = \"svc_a\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        fs::write(dir_a.join("src/lib.rs"), "pub struct ServiceA;\n").unwrap();
        git(&dir_a, &["init", "-q", "-b", "main"]);
        git(&dir_a, &["add", "-A"]);
        git(&dir_a, &["commit", "-q", "-m", "init"]);

        fs::create_dir_all(dir_b.join("src")).unwrap();
        fs::write(
            dir_b.join("Cargo.toml"),
            "[package]\nname = \"svc_b\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        fs::write(dir_b.join("src/lib.rs"), "pub struct ServiceB;\n").unwrap();
        git(&dir_b, &["init", "-q", "-b", "feature"]);
        git(&dir_b, &["add", "-A"]);
        git(&dir_b, &["commit", "-q", "-m", "init"]);

        let server = ContextInitServer::new(None);
        // Initialize knowledge base in dir_a only
        server
            .init(Parameters(McpInitParams {
                path: Some(dir_a.to_string_lossy().to_string()),
                ..McpInitParams::default()
            }))
            .await
            .expect("init dir_a");

        let out = server
            .workspace(Parameters(McpWorkspaceParams {
                path: None,
                paths: Some(vec![
                    dir_a.to_string_lossy().to_string(),
                    dir_b.to_string_lossy().to_string(),
                    missing.to_string_lossy().to_string(),
                ]),
                cursor: None,
            }))
            .await
            .expect("workspace with paths");

        assert!(
            out.contains("dir_a/service"),
            "must disambiguate name: {out}"
        );
        assert!(
            out.contains("dir_b/service"),
            "must disambiguate name: {out}"
        );
        assert!(out.contains("Ok"), "dir_a must be Ok: {out}");
        assert!(
            out.contains("Uninitialized"),
            "dir_b must be Uninitialized: {out}"
        );
        assert!(
            out.contains("MissingPath"),
            "missing must be MissingPath: {out}"
        );
    }

    #[tokio::test]
    async fn mcp_resolves_sibling_repo_when_relative_to_parent() {
        let tmp = TempDirGuard::new("ws_sibling");
        let repo_a = tmp.path().join("repo_a");
        let repo_b = tmp.path().join("repo_b");
        for r in [&repo_a, &repo_b] {
            fs::create_dir_all(r.join("src")).unwrap();
            git(r, &["init", "-q"]);
        }
        let server = ContextInitServer::new(Some(repo_a.clone()));
        let resolved = server.resolve_project_root(None, Some("repo_b")).await;
        assert_eq!(resolved, fs::canonicalize(&repo_b).unwrap());
    }

    struct ScriptedSearch {
        reply: Option<String>,
    }

    impl ci_llm::LlmProvider for ScriptedSearch {
        fn complete(&self, req: &ci_llm::LlmRequest) -> anyhow::Result<ci_llm::LlmResponse> {
            let content = self
                .reply
                .clone()
                .ok_or_else(|| anyhow::anyhow!("scripted provider down"))?;
            Ok(ci_llm::LlmResponse {
                content,
                model: "scripted".to_string(),
                provider_kind: ci_core::config::ModelKind::Ollama,
                alias: "scripted".to_string(),
                tokens_in: req.prompt.len() / 4,
                tokens_out: 1,
                latency_ms: 1,
                retries: 0,
                fallback_used: false,
                work_package: None,
            })
        }

        fn capabilities(&self) -> ci_llm::ProviderCapabilities {
            ci_llm::ProviderCapabilities::default()
        }

        fn kind(&self) -> ci_core::config::ModelKind {
            ci_core::config::ModelKind::Ollama
        }

        fn alias(&self) -> &str {
            "scripted"
        }
    }

    fn search_router(project: &ResolvedProject, reply: Option<&str>) -> ci_llm::RoleRouter {
        let mut cfg = project.config.clone();
        cfg.models.insert(
            "scripted".to_string(),
            ci_core::config::ModelConfig {
                kind: ci_core::config::ModelKind::Ollama,
                model: Some("m".to_string()),
                endpoint: None,
                command: None,
                timeout_ms: None,
                max_input_tokens: None,
            },
        );
        cfg.roles.search = vec!["scripted".to_string()];
        let mut router = ci_llm::RoleRouter::new(cfg, None).unwrap();
        router.register_provider(
            "scripted",
            ScriptedSearch {
                reply: reply.map(str::to_string),
            },
        );
        router
    }

    #[tokio::test]
    async fn search_with_llm_role_reranks_top_hits() {
        let tmp = TempDirGuard::new("search_rerank");
        write_fixture(tmp.path());
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));
        server
            .init(Parameters(init_params(tmp.path())))
            .await
            .expect("init");
        let mut project = server
            .resolve_context(Some(&tmp.path().to_string_lossy()), None)
            .await;
        project.config.roles.search = vec!["scripted".to_string()];

        let first_id = |text: &str| -> String {
            text.lines()
                .find_map(|l| l.strip_prefix("### `"))
                .and_then(|l| l.split('`').next())
                .unwrap_or_default()
                .to_string()
        };

        // Baseline order without LLM reranking (role = none).
        let mut baseline_project = project.clone();
        baseline_project.config.roles.search = vec!["none".to_string()];
        let none_router = ci_llm::RoleRouter::new(baseline_project.config.clone(), None).unwrap();
        let baseline = ContextHandler::search(
            &baseline_project,
            "run",
            None,
            &none_router,
            &TelemetryScope::disabled(),
        )
        .unwrap();
        let baseline_first = first_id(&baseline);
        assert!(!baseline_first.is_empty(), "{baseline}");

        // The scripted reranker puts another subsystem first.
        let ids: Vec<String> = project
            .load_l0_index()
            .unwrap()
            .subsystems
            .iter()
            .map(|s| s.id.clone())
            .collect();
        let promoted = ids
            .iter()
            .find(|i| **i != baseline_first && i.contains("store"))
            .unwrap()
            .clone();
        let reply = format!("{{\"ranked_ids\": [\"{promoted}\"]}}");
        let router = search_router(&project, Some(&reply));
        let reranked =
            ContextHandler::search(&project, "run", None, &router, &TelemetryScope::disabled())
                .unwrap();
        assert_eq!(first_id(&reranked), promoted, "{reranked}");

        // Later pages keep the BM25 order: no LLM call, so paging stays consistent.
        let paged = ContextHandler::search(
            &project,
            "run",
            Some("zz"),
            &router,
            &TelemetryScope::disabled(),
        )
        .unwrap();
        assert_ne!(first_id(&paged), promoted, "{paged}");

        // A multi-line query cannot inject Markdown structure into the header.
        let injected = ContextHandler::search(
            &baseline_project,
            "run\n## injected",
            None,
            &none_router,
            &TelemetryScope::disabled(),
        )
        .unwrap();
        assert!(!injected.contains("\n## injected"), "{injected}");

        // A result that spans several pages is never reranked: page 1 must stay the head of
        // the BM25 order, otherwise pages would duplicate or skip hits.
        let mut tiny = project.clone();
        tiny.config.max_response_tokens = 120;
        let page1 =
            ContextHandler::search(&tiny, "run", None, &router, &TelemetryScope::disabled())
                .unwrap();
        assert!(
            page1.contains("next_cursor") || page1.contains("cursor"),
            "{page1}"
        );
        assert_eq!(first_id(&page1), baseline_first, "{page1}");

        // A failing or garbage provider falls back to the BM25 order.
        for bad in [None, Some("not json at all")] {
            let router = search_router(&project, bad);
            let out =
                ContextHandler::search(&project, "run", None, &router, &TelemetryScope::disabled())
                    .unwrap();
            assert_eq!(first_id(&out), baseline_first, "{out}");
        }
    }

    #[test]
    fn check_drift_errors_on_missing_index_with_mismatch_hint() {
        let tmp = TempDirGuard::new("check_no_kb");
        write_fixture(tmp.path());
        let project = ResolvedProject::load(tmp.path().to_path_buf()).unwrap();
        let err = ContextHandler::check_drift(&project, None).unwrap_err();
        assert!(
            err.to_string().contains("Knowledge base not found"),
            "{err}"
        );

        // Base exists under the other storage mode: the error carries the migrate hint.
        let other = project.knowledge_dir_for(ci_core::config::StorageMode::Central);
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join("L0_index.json"), "{}").unwrap();
        let err = ContextHandler::check_drift(&project, None).unwrap_err();
        assert!(err.to_string().contains("migrate --to"), "{err}");
        let _ = fs::remove_dir_all(&other);
    }

    #[tokio::test]
    async fn init_update_replies_have_footer_when_paged() {
        let tmp = TempDirGuard::new("init_paged");
        write_fixture(tmp.path());
        fs::write(
            tmp.path().join(".context-init.toml"),
            format!("max_response_tokens = 20\n{AGENT_CONFIG}"),
        )
        .unwrap();
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));
        let reply = server
            .init(Parameters(init_params(tmp.path())))
            .await
            .expect("init");
        assert!(reply.contains("[truncated: true"), "{reply}");
        let upd = server
            .update(Parameters(McpUpdateParams {
                path: Some(tmp.path().to_string_lossy().to_string()),
                ..McpUpdateParams::default()
            }))
            .await
            .expect("update");
        assert!(
            upd.contains("tokens:"),
            "update reply must carry a footer: {upd}"
        );
    }

    #[tokio::test]
    async fn pack_all_over_limit_is_refused() {
        let tmp = TempDirGuard::new("pack_all_limit");
        seed_in_repo_knowledge(tmp.path());
        fs::create_dir_all(tmp.path().join("src")).unwrap();
        let big: String = (0..200)
            .map(|i| format!("pub struct Item{i} {{ pub field_{i}: u64 }}\n"))
            .collect();
        fs::write(tmp.path().join("src/lib.rs"), big).unwrap();
        fs::write(
            tmp.path().join(".context-init.toml"),
            "max_response_tokens = 64\n",
        )
        .unwrap();
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));

        let out = server
            .pack(Parameters(McpPackParams {
                subsystem_id: "all".to_string(),
                path: Some(tmp.path().to_string_lossy().to_string()),
                project: None,
                compress: Some(false),
                style: Some("xml".to_string()),
                cursor: None,
            }))
            .await
            .expect("pack tool call");
        assert!(out.contains("pack all"), "{out}");
        assert!(out.contains("per subsystem"), "{out}");
        assert!(out.contains("core"), "{out}");
        assert!(!out.contains("pub struct Item"), "{out}");
    }

    #[tokio::test]
    async fn submit_rejects_unknown_package_id() {
        let tmp = TempDirGuard::new("submit_unknown_pkg");
        let kd = seed_in_repo_knowledge(tmp.path());
        let before = fs::read_to_string(kd.join("L0_index.json")).unwrap();
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));

        let out = server
            .submit(Parameters(McpSubmitParams {
                package_id: "pkg-forged-init-9".to_string(),
                submission: json!({"subsystems": [{
                    "id": "core", "name": "Core", "summary": "forged", "tags": []
                }]}),
                path: Some(tmp.path().to_string_lossy().to_string()),
                project: None,
            }))
            .await
            .expect("submit tool call");
        let out: SubmitResult = serde_json::from_str(&out).unwrap();
        assert!(!out.valid);
        assert!(
            out.errors[0].contains("Unknown package_id"),
            "{:?}",
            out.errors
        );
        assert_eq!(
            fs::read_to_string(kd.join("L0_index.json")).unwrap(),
            before
        );
    }

    #[tokio::test]
    async fn get_subsystem_does_not_read_l1_path_outside_knowledge_dir() {
        let tmp = TempDirGuard::new("l1_escape");
        let kd = seed_in_repo_knowledge(tmp.path());
        fs::write(tmp.path().join("secret.md"), "TOP-SECRET").unwrap();
        let mut index = L0Index::load_from_file(&kd.join("L0_index.json")).unwrap();
        index.subsystems[0].l1_path = "../../secret.md".to_string();
        index.save_to_file(&kd.join("L0_index.json")).unwrap();
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));
        let out = server
            .get_subsystem(Parameters(McpGetSubsystemParams {
                subsystem_id: "core".to_string(),
                path: Some(tmp.path().to_string_lossy().to_string()),
                ..McpGetSubsystemParams::default()
            }))
            .await
            .expect("get_subsystem");
        assert!(!out.contains("TOP-SECRET"), "{out}");
    }

    #[tokio::test]
    async fn synthesized_card_counts_tokens_live() {
        let tmp = TempDirGuard::new("card_tokens");
        let kd = seed_in_repo_knowledge(tmp.path());
        fs::create_dir_all(tmp.path().join("src")).unwrap();
        fs::write(
            tmp.path().join("src/lib.rs"),
            "pub fn answer() -> u32 { 42 }\n".repeat(20),
        )
        .unwrap();
        let index = L0Index::load_from_file(&kd.join("L0_index.json")).unwrap();
        let root = fs::canonicalize(tmp.path()).unwrap();
        let expected = ci_core::analysis::subsystems::subsystem_tokens(&root, &index.subsystems[0]);
        assert!(expected > 0);

        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));
        let out = server
            .get_subsystem(Parameters(McpGetSubsystemParams {
                subsystem_id: "core".to_string(),
                path: Some(tmp.path().to_string_lossy().to_string()),
                ..McpGetSubsystemParams::default()
            }))
            .await
            .expect("get_subsystem");
        assert!(
            out.contains(&format!("**Tokens:** ~{expected} BPE")),
            "{out}"
        );
    }

    #[tokio::test]
    async fn get_subsystems_reports_live_tokens() {
        let tmp = TempDirGuard::new("list_tokens");
        let kd = seed_in_repo_knowledge(tmp.path());
        fs::create_dir_all(tmp.path().join("src")).unwrap();
        fs::write(tmp.path().join("src/lib.rs"), "pub fn a() {}\n".repeat(30)).unwrap();
        let index = L0Index::load_from_file(&kd.join("L0_index.json")).unwrap();
        let root = fs::canonicalize(tmp.path()).unwrap();
        let expected = ci_core::analysis::subsystems::subsystem_tokens(&root, &index.subsystems[0]);
        assert!(expected > 0);
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));
        let out = server
            .get_subsystems(Parameters(McpGetSubsystemsParams {
                path: Some(tmp.path().to_string_lossy().to_string()),
                ..McpGetSubsystemsParams::default()
            }))
            .await
            .expect("get_subsystems");
        let json_part = out.split("\n\n[").next().unwrap();
        let v: serde_json::Value = serde_json::from_str(json_part).unwrap();
        assert_eq!(v[0]["tokens"], expected);
    }

    #[tokio::test]
    async fn get_subsystems_fits_65_entries_and_escapes_multiline_summary() {
        let tmp = TempDirGuard::new("subsystems_65");
        let kd = tmp.path().join(".context-init").join("knowledge");
        fs::create_dir_all(kd.join("subsystems")).unwrap();
        let mut subs = Vec::new();
        for i in 0..65 {
            let files = (0..10)
                .map(|j| SubsystemFile {
                    path: format!("src/sub_{i}/file_{j}.rs"),
                    is_entrypoint: j == 0,
                    lines: 20,
                })
                .collect();
            let key_symbols = (0..10).map(|j| format!("Sym{j}")).collect();
            let summary = if i == 0 {
                "Multiline summary with \"quotes\"\nand second line.".to_string()
            } else {
                format!("Subsystem {i} description")
            };
            subs.push(SubsystemInfo {
                id: format!("sub_{i}"),
                name: format!("Subsystem {i}"),
                summary,
                l1_path: format!("subsystems/sub_{i}.md"),
                tags: vec!["tag1".to_string(), "tag2".to_string()],
                entrypoints: vec![format!("src/sub_{i}/file_0.rs")],
                key_files: vec![format!("src/sub_{i}/file_0.rs")],
                files,
                key_symbols,
                test_command: None,
                tokens: 150,
                parent: None,
                summary_source: ci_core::SummarySource::Heuristic,
                summary_model: None,
                needs_review: false,
            });
        }
        let index = L0Index {
            subsystems: subs,
            ..L0Index::default()
        };
        index.save_to_file(&kd.join("L0_index.json")).unwrap();

        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));
        let out = server
            .get_subsystems(Parameters(McpGetSubsystemsParams {
                path: Some(tmp.path().to_string_lossy().to_string()),
                ..McpGetSubsystemsParams::default()
            }))
            .await
            .expect("get_subsystems");

        assert!(!out.contains("truncated: true"), "{out}");
        assert!(out.len() < 9_000, "bytes={}", out.len());
        let json_part = out.split("\n\n[").next().unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(json_part).unwrap();
        assert_eq!(parsed.len(), 65);
        assert_eq!(parsed[0]["file_count"], 10);
        assert!(parsed[0].get("files").is_none());
        assert!(parsed[0].get("key_symbols").is_none());
        assert!(parsed[0].get("entrypoints").is_none());
    }

    #[tokio::test]
    async fn submit_rejects_traversal_ids() {
        let tmp = TempDirGuard::new("submit_traversal");
        let kd = seed_in_repo_knowledge(tmp.path());
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));

        let upd = submit_with_package(
            &server,
            tmp.path(),
            RoleName::Init,
            json!({"subsystems": [{
                "id": "../escape", "name": "x", "summary": "s", "tags": []
            }]}),
        )
        .await;
        assert!(!upd.valid);
        assert!(
            upd.errors
                .iter()
                .any(|e| e.contains("../escape") && e.contains(SUBSYSTEM_ID_PATTERN)),
            "{:?}",
            upd.errors
        );
        assert!(!kd.join("escape.md").exists());
        assert!(!kd.parent().unwrap().join("escape.md").exists());

        let abs_target = tmp.path().join("abs_target");
        let init = submit_with_package(
            &server,
            tmp.path(),
            RoleName::Init,
            json!({"subsystems": [{
                "id": abs_target.to_string_lossy(), "name": "x", "summary": "s", "tags": []
            }]}),
        )
        .await;
        assert!(!init.valid);
        assert!(
            init.errors.iter().any(|e| e.contains("id")),
            "{:?}",
            init.errors
        );
        assert!(!tmp.path().join("abs_target.md").exists());
    }

    #[tokio::test]
    async fn submit_ignores_submitted_files_list() {
        let tmp = TempDirGuard::new("submit_files");
        let kd = seed_in_repo_knowledge(tmp.path());
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));

        let out = submit_with_package(
            &server,
            tmp.path(),
            RoleName::Init,
            json!({"subsystems": [{
                "id": "core", "name": "Core", "summary": "Updated summary", "tags": ["x"],
                "files": [{"path": "/etc/passwd", "is_entrypoint": false, "lines": 1}]
            }]}),
        )
        .await;
        assert!(out.valid, "{:?}", out.errors);
        let saved = L0Index::load_from_file(&kd.join("L0_index.json")).unwrap();
        let core = saved.subsystems.iter().find(|s| s.id == "core").unwrap();
        assert_eq!(core.summary, "Updated summary");
        assert_eq!(core.files.len(), 1);
        assert_eq!(core.files[0].path, "src/lib.rs");
    }

    /// Review Focus 5: a write failure midway (the card is staged, `L0_index.md` cannot be)
    /// leaves the index and every card byte-for-byte unchanged.
    #[cfg(unix)]
    #[tokio::test]
    async fn submit_reports_write_errors() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = TempDirGuard::new("submit_write_err");
        isolate_config_dirs(&tmp);
        write_fixture(tmp.path());
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));
        let package_id = init_with_agent_package(&server, tmp.path()).await;
        let resolved = server
            .resolve_context(Some(&tmp.path().to_string_lossy()), None)
            .await;
        let kd = resolved.knowledge_dir.clone();
        let target = resolved.load_l0_index().unwrap().subsystems[0].clone();
        let index_before = fs::read(kd.join("L0_index.json")).unwrap();
        let card_before = fs::read(kd.join(&target.l1_path)).unwrap();

        /// Restores write access even when an assertion panics.
        struct ReadOnly<'a>(&'a Path);
        impl Drop for ReadOnly<'_> {
            fn drop(&mut self) {
                let _ = fs::set_permissions(self.0, fs::Permissions::from_mode(0o755));
            }
        }
        fs::set_permissions(&kd, fs::Permissions::from_mode(0o555)).unwrap();
        let read_only = ReadOnly(&kd);
        if fs::write(kd.join(".probe"), "").is_ok() {
            // Running with privileges that ignore permissions: nothing to observe.
            return;
        }
        let out = submit_payload(
            &server,
            tmp.path(),
            &package_id,
            json!({"subsystems": [{
                "id": target.id, "name": "N", "summary": "Never written", "tags": []
            }]}),
        )
        .await;
        drop(read_only);

        assert!(!out.valid, "{out:?}");
        assert!(!out.errors.is_empty());
        assert!(!out.message.contains("accepted"), "{}", out.message);
        assert_eq!(fs::read(kd.join("L0_index.json")).unwrap(), index_before);
        assert_eq!(fs::read(kd.join(&target.l1_path)).unwrap(), card_before);
        let leftovers: Vec<_> = fs::read_dir(kd.join("subsystems"))
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    #[tokio::test]
    async fn context_submit_valid_and_schema_error_feedback() {
        let tmp = TempDirGuard::new("submit");
        isolate_config_dirs(&tmp);
        write_fixture(tmp.path());
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));
        server
            .init(Parameters(init_params(tmp.path())))
            .await
            .expect("init");
        let resolved = server
            .resolve_context(Some(&tmp.path().to_string_lossy()), None)
            .await;
        let id = resolved.load_l0_index().unwrap().subsystems[0].id.clone();

        let pkg = AgentWorkPackage {
            package_id: "pkg-worker-init-1001".to_string(),
            role: RoleName::Init,
            facts: json!({}),
            instruction: "Extract subsystems".to_string(),
            json_schema: ci_llm::default_role_json_schema(RoleName::Init),
            alias: "worker".to_string(),
            deep: false,
            agents_md: false,
            project_root: Some(resolved.root.clone()),
        };
        server.register_work_package(pkg.clone()).await;

        // 1. Valid submission matching RoleName::Init schema
        let valid_payload = json!({
            "subsystems": [
                {
                    "id": id,
                    "name": "MCP Server",
                    "summary": "Protocol server implementation",
                    "tags": ["mcp", "rust"]
                }
            ]
        });
        let res_valid = server
            .submit(Parameters(McpSubmitParams {
                package_id: pkg.package_id.clone(),
                submission: valid_payload,
                path: None,
                project: None,
            }))
            .await;
        assert!(
            res_valid.is_ok(),
            "Submit valid failed: {:?}",
            res_valid.err()
        );
        let valid_out: SubmitResult =
            serde_json::from_str(&res_valid.unwrap()).expect("parse SubmitResult JSON");
        assert!(valid_out.valid, "{:?}", valid_out.errors);
        assert_eq!(valid_out.package_id, pkg.package_id);
        assert!(valid_out.errors.is_empty());
        assert!(valid_out.message.contains("validated and accepted"));

        // 2. Invalid submission missing required `summary` field (the accepted submission
        // consumed the package, so it is issued again)
        server.register_work_package(pkg.clone()).await;
        let invalid_payload = json!({
            "subsystems": [
                {
                    "id": "ci_mcp",
                    "name": "MCP Server"
                }
            ]
        });
        let res_invalid = server
            .submit(Parameters(McpSubmitParams {
                package_id: pkg.package_id.clone(),
                submission: invalid_payload,
                path: None,
                project: None,
            }))
            .await;
        assert!(
            res_invalid.is_ok(),
            "Submit tool call should succeed returning error details"
        );
        let invalid_out: SubmitResult =
            serde_json::from_str(&res_invalid.unwrap()).expect("parse SubmitResult JSON");
        assert!(!invalid_out.valid);
        assert!(!invalid_out.errors.is_empty());
        assert!(invalid_out.message.contains("rejected"));
        assert!(
            invalid_out.errors.iter().any(|e| e.contains("summary")),
            "errors must name the missing field: {:?}",
            invalid_out.errors
        );
    }

    #[tokio::test]
    async fn context_feedback_records_signal_with_hashed_project_id() {
        let tmp = TempDirGuard::new("feedback");
        // Project telemetry off and config dirs isolated: only the feedback event is expected.
        let (server, sink) = telemetry_server(&tmp, false);

        let res = server
            .feedback(Parameters(McpFeedbackParams {
                tool: "context_pack".to_string(),
                verdict: "helpful".to_string(),
                note: Some("Clean AST token compression".to_string()),
                path: Some(tmp.path().to_string_lossy().to_string()),
                project: None,
            }))
            .await;
        assert!(res.is_ok(), "Feedback failed: {:?}", res.err());
        let out: FeedbackResult =
            serde_json::from_str(&res.unwrap()).expect("parse FeedbackResult JSON");
        assert_eq!(out.status, "ok");
        assert_eq!(out.tool, "context_pack");
        assert_eq!(out.verdict, "helpful");
        assert!(out.recorded);

        let recorded_events = sink.events.lock().unwrap().clone();
        assert_eq!(recorded_events.len(), 1);
        let ev = &recorded_events[0];
        assert_eq!(ev.project_id_hash.len(), 64);
        assert!(ev.project_id_hash.chars().all(|c| c.is_ascii_hexdigit()));

        match &ev.signal {
            TelemetrySignal::Feedback(fb) => {
                assert_eq!(fb.tool, "context_pack");
                assert_eq!(fb.verdict, "helpful");
                assert_eq!(fb.note.as_deref(), Some("Clean AST token compression"));
            }
            _ => panic!("Expected TelemetrySignal::Feedback"),
        }
    }

    #[tokio::test]
    async fn tool_pack_all_and_token_budget_truncation() {
        let tmp = TempDirGuard::new("pack_budget");
        let src_dir = tmp.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();

        // Write sample Rust code
        fs::write(
            src_dir.join("main.rs"),
            "fn main() {\n    println!(\"Hello\");\n}\n",
        )
        .unwrap();
        fs::write(
            src_dir.join("lib.rs"),
            "pub fn process(data: &[u8]) -> usize {\n    let mut x = 0;\n    for b in data { x += *b as usize; }\n    x\n}\n",
        )
        .unwrap();

        // Write .context-init.toml with a tight max_response_tokens limit
        fs::write(
            tmp.path().join(".context-init.toml"),
            "max_response_tokens = 60\n",
        )
        .unwrap();

        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));

        // 1. Initialize KB
        let init_res = server
            .init(Parameters(McpInitParams {
                path: Some(tmp.path().to_string_lossy().to_string()),
                deep: Some(false),
                model: None,
                force: Some(true),
                with_agents_md: Some(false),
                project: None,
            }))
            .await;
        assert!(init_res.is_ok());

        // Update L0 index files so pack finds files
        let resolved = server.resolve_context(None, None).await;
        let mut idx = resolved.load_l0_index().unwrap();
        idx.subsystems[0].files = vec![
            SubsystemFile {
                path: "src/main.rs".to_string(),
                is_entrypoint: true,
                lines: 3,
            },
            SubsystemFile {
                path: "src/lib.rs".to_string(),
                is_entrypoint: false,
                lines: 5,
            },
        ];
        idx.save_to_file(&resolved.knowledge_dir.join("L0_index.json"))
            .unwrap();

        // 2. Call context_pack with subsystem_id = "all"
        let pack_page1 = server
            .pack(Parameters(McpPackParams {
                subsystem_id: "all".to_string(),
                path: Some(tmp.path().to_string_lossy().to_string()),
                project: None,
                compress: Some(true),
                style: Some("xml".to_string()),
                cursor: None,
            }))
            .await
            .expect("pack page 1 succeeds");

        // The budget is 60 tokens, so it MUST truncate and contain next_cursor
        assert!(
            pack_page1.contains("[truncated: true | next_cursor: \"cur_"),
            "Expected truncated footer in page 1, got: {pack_page1}"
        );
        let tokens_p1 = count_tokens(&pack_page1);
        assert!(
            tokens_p1 <= 60,
            "Page 1 tokens {} exceeded budget 60",
            tokens_p1
        );

        // Extract cursor from footer
        let start_pat = "next_cursor: \"";
        let start_pos = pack_page1.find(start_pat).unwrap() + start_pat.len();
        let end_pos = pack_page1[start_pos..].find('"').unwrap() + start_pos;
        let next_cur = &pack_page1[start_pos..end_pos];

        // 3. Request page 2 with next_cursor
        let pack_page2 = server
            .pack(Parameters(McpPackParams {
                subsystem_id: "all".to_string(),
                path: Some(tmp.path().to_string_lossy().to_string()),
                project: None,
                compress: Some(true),
                style: Some("xml".to_string()),
                cursor: Some(next_cur.to_string()),
            }))
            .await
            .expect("pack page 2 succeeds");

        let tokens_p2 = count_tokens(&pack_page2);
        assert!(
            tokens_p2 <= 60,
            "Page 2 tokens {} exceeded budget 60",
            tokens_p2
        );
    }

    #[tokio::test]
    async fn monorepo_workspace_root_resolution() {
        let tmp = TempDirGuard::new("workspace_root");
        let ws_root = tmp.path().join("my_workspace");
        let subcrate = ws_root.join("crates").join("sub_crate");
        fs::create_dir_all(subcrate.join("src")).unwrap();

        // Workspace root contains Cargo.toml with [workspace] and .git marker
        fs::write(
            ws_root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\n",
        )
        .unwrap();
        fs::create_dir_all(ws_root.join(".git")).unwrap();

        // Subcrate contains member Cargo.toml without [workspace]
        fs::write(
            subcrate.join("Cargo.toml"),
            "[package]\nname = \"sub_crate\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        // Climbing from subcrate/src/lib.rs MUST resolve to ws_root, NOT subcrate!
        let nested_file = subcrate.join("src").join("lib.rs");
        fs::write(&nested_file, "pub fn hello() {}\n").unwrap();

        let resolved = find_project_root_upward(&nested_file);
        assert_eq!(
            resolved,
            Some(fs::canonicalize(&ws_root).unwrap()),
            "find_project_root_upward must find workspace root over member crate"
        );
    }

    #[tokio::test]
    async fn context_task_maps_paths_to_subsystems() {
        let tmp = TempDirGuard::new("task_paths");
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));

        // Initialize KB with a known subsystem having files
        let _ = server
            .init(Parameters(McpInitParams {
                path: Some(tmp.path().to_string_lossy().to_string()),
                ..Default::default()
            }))
            .await;

        let resolved = server
            .resolve_context(Some(&tmp.path().to_string_lossy()), None)
            .await;
        let mut idx = resolved.load_l0_index().unwrap();
        idx.subsystems[0].files = vec![SubsystemFile {
            path: "src/engine.rs".to_string(),
            is_entrypoint: false,
            lines: 50,
        }];
        idx.save_to_file(&resolved.knowledge_dir.join("L0_index.json"))
            .unwrap();

        // Call context_task passing task-specific path
        let task_res = server
            .task(Parameters(McpTaskParams {
                paths: Some(vec!["src/engine.rs".to_string()]),
                path: Some(tmp.path().to_string_lossy().to_string()),
                ..Default::default()
            }))
            .await
            .expect("context_task succeeds");

        assert!(task_res.contains("## Task-Specified Paths"));
        assert!(task_res.contains("`src/engine.rs` (subsystem: `core`)"));
    }

    // --- operational telemetry instrumentation (Task 2) ---

    /// Isolates data/config dirs, writes the project config and returns a server with a mock sink.
    fn telemetry_server(
        tmp: &TempDirGuard,
        project_enabled: bool,
    ) -> (ContextInitServer, Arc<MockTelemetrySink>) {
        std::env::set_var("XDG_CONFIG_HOME", tmp.path().join(".xdg-config"));
        std::env::set_var("HOME", tmp.path().join(".home"));
        fs::write(
            tmp.path().join(".context-init.toml"),
            format!("[telemetry]\nenabled = {project_enabled}\n"),
        )
        .unwrap();
        let sink = Arc::new(MockTelemetrySink::default());
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()))
            .with_telemetry_sink(sink.clone());
        (server, sink)
    }

    fn operational(sink: &MockTelemetrySink) -> Vec<ci_core::telemetry::OperationalSignal> {
        sink.events
            .lock()
            .unwrap()
            .iter()
            .filter_map(|e| match &e.signal {
                TelemetrySignal::Operational(op) => Some(op.clone()),
                _ => None,
            })
            .collect()
    }

    #[tokio::test]
    async fn every_tool_records_one_operational_event() {
        let tmp = TempDirGuard::new("tel_all");
        let (server, sink) = telemetry_server(&tmp, true);
        let path = Some(tmp.path().to_string_lossy().to_string());

        let _ = server
            .check_drift(Parameters(McpCheckParams {
                path: path.clone(),
                ..Default::default()
            }))
            .await;
        let _ = server
            .get_subsystems(Parameters(McpGetSubsystemsParams {
                path: path.clone(),
                ..Default::default()
            }))
            .await;
        let _ = server
            .get_subsystem(Parameters(McpGetSubsystemParams {
                subsystem_id: "nope".to_string(),
                path: path.clone(),
                ..Default::default()
            }))
            .await;
        let _ = server
            .pack(Parameters(McpPackParams {
                subsystem_id: "nope".to_string(),
                path: path.clone(),
                ..Default::default()
            }))
            .await;
        let _ = server
            .submit(Parameters(McpSubmitParams {
                package_id: "unknown-package".to_string(),
                submission: json!({}),
                path: path.clone(),
                project: None,
            }))
            .await;
        let _ = server
            .feedback(Parameters(McpFeedbackParams {
                tool: "context_pack".to_string(),
                verdict: "helpful".to_string(),
                note: None,
                path: path.clone(),
                project: None,
            }))
            .await;
        let _ = server
            .init(Parameters(McpInitParams {
                path: path.clone(),
                deep: Some(false),
                force: Some(true),
                with_agents_md: Some(false),
                ..Default::default()
            }))
            .await;
        let _ = server
            .update(Parameters(McpUpdateParams {
                path: path.clone(),
                ..Default::default()
            }))
            .await;
        let _ = server
            .workspace(Parameters(McpWorkspaceParams {
                path: path.clone(),
                ..Default::default()
            }))
            .await;
        let _ = server
            .scan(Parameters(McpScanParams {
                path: path.clone(),
                ..Default::default()
            }))
            .await;
        let _ = server
            .task(Parameters(McpTaskParams {
                path: path.clone(),
                ..Default::default()
            }))
            .await;
        let _ = server
            .search(Parameters(McpSearchParams {
                query: "core".to_string(),
                path,
                ..Default::default()
            }))
            .await;

        let mut names: Vec<String> = operational(&sink).into_iter().map(|o| o.tool).collect();
        names.sort();
        assert_eq!(
            names,
            vec![
                "context_check",
                "context_feedback",
                "context_get_subsystem",
                "context_get_subsystems",
                "context_init",
                "context_pack",
                "context_scan",
                "context_search",
                "context_submit",
                "context_task",
                "context_update",
                "context_workspace",
            ]
        );
        assert!(operational(&sink)
            .iter()
            .all(|o| o.channel == "mcp" && !o.nested));
    }

    #[tokio::test]
    async fn client_info_sets_harness() {
        let tmp = TempDirGuard::new("tel_harness");
        let (server, sink) = telemetry_server(&tmp, true);
        let params = || McpScanParams {
            path: Some(tmp.path().to_string_lossy().to_string()),
            ..Default::default()
        };

        // No clientInfo seen: unknown client => `other`.
        let _ = server.scan(Parameters(params())).await;
        assert_eq!(operational(&sink)[0].harness.as_deref(), Some("other"));

        *server.client_harness.write().unwrap() =
            ci_core::telemetry::harness_from_client_name(Some("claude-code"));
        let _ = server.scan(Parameters(params())).await;
        assert_eq!(
            operational(&sink)[1].harness.as_deref(),
            Some("claude-code")
        );
    }

    #[tokio::test]
    async fn project_disabled_telemetry_writes_nothing() {
        let tmp = TempDirGuard::new("tel_off");
        let (server, sink) = telemetry_server(&tmp, false);
        let _ = server
            .scan(Parameters(McpScanParams {
                path: Some(tmp.path().to_string_lossy().to_string()),
                ..Default::default()
            }))
            .await;
        assert!(sink.events.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn failed_tool_records_error_class() {
        let tmp = TempDirGuard::new("tel_err");
        let (server, sink) = telemetry_server(&tmp, true);
        let res = server
            .get_subsystem(Parameters(McpGetSubsystemParams {
                subsystem_id: "does-not-exist".to_string(),
                path: Some(tmp.path().to_string_lossy().to_string()),
                ..Default::default()
            }))
            .await;
        assert!(res.is_err());
        let ops = operational(&sink);
        assert_eq!(ops.len(), 1);
        assert!(!ops[0].success);
        assert_eq!(ops[0].error_class.as_deref(), Some("not_found"));
    }

    // --- efficiency telemetry (Task 3) ---

    fn efficiency(
        sink: &MockTelemetrySink,
        tool: &str,
    ) -> Vec<ci_core::telemetry::EfficiencySignal> {
        sink.events
            .lock()
            .unwrap()
            .iter()
            .filter_map(|e| match &e.signal {
                TelemetrySignal::Efficiency(ef) if ef.tool == tool => Some(ef.clone()),
                _ => None,
            })
            .collect()
    }

    /// Project with one real source file, initialized; returns the path argument.
    async fn initialized_project(server: &ContextInitServer, tmp: &TempDirGuard) -> Option<String> {
        fs::create_dir_all(tmp.path().join("src")).unwrap();
        let body: String = (0..300)
            .map(|i| format!("pub fn handler_{i}(value: usize) -> usize {{ value + {i} }}\n"))
            .collect();
        fs::write(tmp.path().join("src/lib.rs"), body).unwrap();
        let path = Some(tmp.path().to_string_lossy().to_string());
        server
            .init(Parameters(McpInitParams {
                path: path.clone(),
                force: Some(true),
                with_agents_md: Some(false),
                ..Default::default()
            }))
            .await
            .expect("init succeeds");
        path
    }

    #[tokio::test]
    async fn pack_records_efficiency() {
        let tmp = TempDirGuard::new("eff_pack");
        let (server, sink) = telemetry_server(&tmp, true);
        let path = initialized_project(&server, &tmp).await;
        let resolved = server.resolve_context(path.as_deref(), None).await;
        let sub = resolved.load_l0_index().unwrap().subsystems[0].clone();
        let expected = ci_core::analysis::subsystems::subsystem_tokens(&resolved.root, &sub);
        assert!(expected > 0, "fixture must have source tokens");

        server
            .pack(Parameters(McpPackParams {
                subsystem_id: sub.id.clone(),
                path,
                ..Default::default()
            }))
            .await
            .expect("pack succeeds");

        let effs = efficiency(&sink, "context_pack");
        assert_eq!(effs.len(), 1, "{effs:?}");
        assert_eq!(effs[0].source_tokens, expected);
        assert!(effs[0].response_tokens > 0);
        assert_eq!(effs[0].cursor_hops, 0);
        assert!(!effs[0].empty_search);
    }

    #[tokio::test]
    async fn search_empty_sets_flag() {
        let tmp = TempDirGuard::new("eff_search");
        let (server, sink) = telemetry_server(&tmp, true);
        let path = initialized_project(&server, &tmp).await;

        server
            .search(Parameters(McpSearchParams {
                query: "zzqxnomatchtoken".to_string(),
                path,
                ..Default::default()
            }))
            .await
            .expect("search succeeds");

        let effs = efficiency(&sink, "context_search");
        assert_eq!(effs.len(), 1, "{effs:?}");
        assert!(effs[0].empty_search);
        assert_eq!(effs[0].source_tokens, 0);
    }

    #[tokio::test]
    async fn cursor_hops_counted() {
        let tmp = TempDirGuard::new("eff_cursor");
        let (server, sink) = telemetry_server(&tmp, true);
        let path = initialized_project(&server, &tmp).await;
        fs::write(
            tmp.path().join(".context-init.toml"),
            "max_response_tokens = 20\n[telemetry]\nenabled = true\n",
        )
        .unwrap();

        let page1 = server
            .get_subsystems(Parameters(McpGetSubsystemsParams {
                path: path.clone(),
                ..Default::default()
            }))
            .await
            .expect("page 1");
        let cursor = page1
            .split("next_cursor: \"")
            .nth(1)
            .and_then(|r| r.split('"').next())
            .expect("page 1 must be truncated with a cursor")
            .to_string();
        server
            .get_subsystems(Parameters(McpGetSubsystemsParams {
                path,
                cursor: Some(cursor),
                ..Default::default()
            }))
            .await
            .expect("page 2");

        let effs = efficiency(&sink, "context_get_subsystems");
        assert_eq!(effs.len(), 2, "{effs:?}");
        assert_eq!(effs[0].cursor_hops, 0);
        assert!(effs[0].truncated);
        assert_eq!(effs[1].cursor_hops, 1);
    }

    #[test]
    fn tool_descriptions_have_triggers_and_fit() {
        let tools = ContextInitServer::tool_router().list_all();
        assert_eq!(tools.len(), 12, "{tools:?}");
        let routed = [
            "context_search",
            "context_get_subsystems",
            "context_get_subsystem",
            "context_pack",
            "context_task",
            "context_check",
        ];
        for tool in &tools {
            let desc = tool.description.as_deref().unwrap_or_default();
            assert!(
                desc.chars().count() <= 300,
                "{}: {} chars",
                tool.name,
                desc.chars().count()
            );
            if routed.contains(&tool.name.as_ref()) {
                assert!(
                    desc.contains("Use ")
                        || desc.contains("Start here")
                        || desc.contains("Run before"),
                    "{}: no routing trigger in {desc:?}",
                    tool.name
                );
            }
        }
    }
}
