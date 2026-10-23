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
    FeedbackResult, McpCheckParams, McpCheckResult, McpFeedbackParams, McpGetSubsystemParams,
    McpGetSubsystemsParams, McpInitParams, McpPackParams, McpScanParams, McpSearchParams,
    McpSubmitParams, McpTaskParams, McpUpdateParams, SubmitResult,
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
    use ci_core::storage::{L0Index, SubsystemFile, SubsystemInfo, CURRENT_L0_SCHEMA_VERSION};
    use ci_core::telemetry::{TelemetryEvent, TelemetrySignal, TelemetrySink};
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
        assert_eq!(VERSION, "0.3.0");
        assert_eq!(VERSION, ci_core::VERSION);
        assert_eq!(VERSION, ci_llm::VERSION);
        assert!(crate_info().contains("ci-mcp/0.3.0"));
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

    #[tokio::test]
    async fn central_storage_and_lazy_fallback_resolution() {
        let tmp = TempDirGuard::new("kb_store");
        let repo_root = tmp.path().join("my_repo");
        let src_dir = repo_root.join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(repo_root.join("Cargo.toml"), "[package]\nname = \"demo\"\n").unwrap();

        // 1. Unmigrated in-repo fallback: .agents/knowledge/L0_index.json
        let in_repo_kb = repo_root.join(".agents").join("knowledge");
        fs::create_dir_all(&in_repo_kb).unwrap();
        let in_repo_index = L0Index {
            schema_version: 1,
            project: "my_repo".to_string(),
            subsystems: vec![SubsystemInfo {
                id: "core".to_string(),
                name: "Core Logic".to_string(),
                summary: "In-repo fallback subsystem".to_string(),
                l1_path: "subsystems/core.md".to_string(),
                tags: vec!["core".to_string()],
                entrypoints: vec!["src/main.rs".to_string()],
                key_files: vec!["src/lib.rs".to_string()],
                files: vec![SubsystemFile {
                    path: "src/lib.rs".to_string(),
                    is_entrypoint: true,
                    lines: 20,
                }],
                key_symbols: vec!["run".to_string()],
                test_command: None,
                tokens: 50,
            }],
            ..L0Index::default()
        };
        in_repo_index
            .save_to_file(&in_repo_kb.join("L0_index.json"))
            .unwrap();

        let resolved_unmigrated = ResolvedProject::load(repo_root.clone()).unwrap();
        assert!(!resolved_unmigrated.is_central);
        assert_eq!(
            resolved_unmigrated.knowledge_dir,
            fs::canonicalize(&in_repo_kb).unwrap()
        );
        let loaded_fallback = resolved_unmigrated.load_l0_index().unwrap();
        assert_eq!(
            loaded_fallback.subsystems[0].summary,
            "In-repo fallback subsystem"
        );

        // 2. Central storage resolution: when central storage directory exists and has L0_index.json
        let central_dir = resolved_unmigrated
            .storage_paths
            .project_knowledge_dir(&resolved_unmigrated.project_id);
        fs::create_dir_all(&central_dir).unwrap();
        let central_index = L0Index {
            schema_version: CURRENT_L0_SCHEMA_VERSION,
            project: resolved_unmigrated.project_id.clone(),
            subsystems: vec![SubsystemInfo {
                id: "core".to_string(),
                name: "Core Logic".to_string(),
                summary: "Central storage subsystem".to_string(),
                l1_path: "subsystems/core.md".to_string(),
                tags: vec!["central".to_string()],
                entrypoints: vec![],
                key_files: vec![],
                files: vec![],
                key_symbols: vec![],
                test_command: None,
                tokens: 80,
            }],
            ..L0Index::default()
        };
        central_index
            .save_to_file(&central_dir.join("L0_index.json"))
            .unwrap();

        let resolved_central = ResolvedProject::load(repo_root).unwrap();
        assert!(resolved_central.is_central);
        assert_eq!(
            resolved_central.knowledge_dir,
            fs::canonicalize(&central_dir).unwrap()
        );
        let loaded_central = resolved_central.load_l0_index().unwrap();
        assert_eq!(
            loaded_central.subsystems[0].summary,
            "Central storage subsystem"
        );
    }

    /// Seeds an in-repo knowledge base (lazy fallback) with one `core` subsystem; returns its dir.
    fn seed_in_repo_knowledge(root: &Path) -> PathBuf {
        let kd = root.join(".agents").join("knowledge");
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
        let paths = crate::server::roots_to_paths(&[
            root("file:///tmp/a%20b"),
            root("https://example.com/repo"),
            root("file:///srv/%D0%BF%D1%80%D0%BE%D0%B5%D0%BA%D1%82"),
        ]);
        #[cfg(not(windows))]
        assert_eq!(
            paths,
            vec![PathBuf::from("/tmp/a b"), PathBuf::from("/srv/проект")]
        );
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
        assert!(resolved.is_central, "init must write central storage");
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

    #[tokio::test]
    async fn init_update_replies_have_footer_when_paged() {
        let tmp = TempDirGuard::new("init_paged");
        write_fixture(tmp.path());
        fs::write(
            tmp.path().join(".context-init.toml"),
            format!("max_response_tokens = 64\n{AGENT_CONFIG}"),
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
    async fn submit_rejects_traversal_ids() {
        let tmp = TempDirGuard::new("submit_traversal");
        let kd = seed_in_repo_knowledge(tmp.path());
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));

        let upd = submit_with_package(
            &server,
            tmp.path(),
            RoleName::Update,
            json!({"id": "../escape", "summary": "s", "body": "b"}),
        )
        .await;
        assert!(!upd.valid);
        assert!(
            upd.errors.iter().any(|e| e.contains("id")),
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

    #[tokio::test]
    async fn submit_reports_write_errors() {
        let tmp = TempDirGuard::new("submit_write_err");
        let kd = seed_in_repo_knowledge(tmp.path());
        fs::remove_dir_all(kd.join("subsystems")).unwrap();
        fs::write(kd.join("subsystems"), "not a dir").unwrap();
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));

        let out = submit_with_package(
            &server,
            tmp.path(),
            RoleName::Update,
            json!({"id": "core", "summary": "s", "body": "# Core"}),
        )
        .await;
        assert!(!out.valid || !out.errors.is_empty());
        assert!(!out.message.contains("accepted"), "{}", out.message);
    }

    #[tokio::test]
    async fn context_submit_valid_and_schema_error_feedback() {
        let tmp = TempDirGuard::new("submit");
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));

        let pkg = AgentWorkPackage {
            package_id: "pkg-worker-init-1001".to_string(),
            role: RoleName::Init,
            facts: json!({}),
            instruction: "Extract subsystems".to_string(),
            json_schema: ci_llm::default_role_json_schema(RoleName::Init),
        };
        server.register_work_package(pkg.clone()).await;

        // 1. Valid submission matching RoleName::Init schema
        let valid_payload = json!({
            "subsystems": [
                {
                    "id": "ci_mcp",
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
        assert!(valid_out.valid);
        assert_eq!(valid_out.package_id, pkg.package_id);
        assert!(valid_out.errors.is_empty());
        assert!(valid_out.message.contains("validated and accepted"));

        // 2. Invalid submission missing required `summary` field
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
        let sink = Arc::new(MockTelemetrySink::default());
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()))
            .with_telemetry_sink(sink.clone());

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
    async fn context_submit_update_role_persists_card_and_metadata() {
        let tmp = TempDirGuard::new("submit_update");
        let server = ContextInitServer::new(Some(tmp.path().to_path_buf()));

        // 1. Initialize KB
        let _ = server
            .init(Parameters(McpInitParams {
                path: Some(tmp.path().to_string_lossy().to_string()),
                ..Default::default()
            }))
            .await
            .expect("init succeeds");

        // 2. Register work package for Update role
        let pkg = AgentWorkPackage {
            package_id: "pkg-worker-update-2002".to_string(),
            role: RoleName::Update,
            facts: json!({}),
            instruction: "Update core card".to_string(),
            json_schema: ci_llm::default_role_json_schema(RoleName::Update),
        };
        server.register_work_package(pkg.clone()).await;

        // 3. Submit valid update
        let updated_body =
            "# Subsystem: Core Logic (L1)\n\nUpdated card body content from agent.\n";
        let update_payload = json!({
            "id": "core",
            "summary": "Updated core summary by agent",
            "body": updated_body,
            "tags": ["core", "updated"]
        });

        let res = server
            .submit(Parameters(McpSubmitParams {
                package_id: pkg.package_id.clone(),
                submission: update_payload,
                path: Some(tmp.path().to_string_lossy().to_string()),
                project: None,
            }))
            .await
            .expect("submit succeeds");

        let out: SubmitResult = serde_json::from_str(&res).unwrap();
        assert!(out.valid);
        assert!(out.errors.is_empty());

        // Verify disk card file was updated
        let resolved = server
            .resolve_context(Some(&tmp.path().to_string_lossy()), None)
            .await;
        let card_p = resolved.knowledge_dir.join("subsystems").join("core.md");
        assert!(card_p.is_file(), "core.md must exist on disk");
        let card_content = fs::read_to_string(&card_p).unwrap();
        assert_eq!(card_content, updated_body);

        // Verify L0_index.json metadata was updated
        let idx = resolved.load_l0_index().unwrap();
        let core_sub = idx.subsystems.iter().find(|s| s.id == "core").unwrap();
        assert_eq!(core_sub.summary, "Updated core summary by agent");
        assert_eq!(
            core_sub.tags,
            vec!["core".to_string(), "updated".to_string()]
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
}
