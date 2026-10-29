//! Single global MCP server, cascade project root resolution, and stdio runner (D20, D21, R3, R5).

use anyhow::Result;
use rmcp::handler::server::wrapper::Parameters;
#[allow(deprecated)] // MCP roots: deprecated by SEP-2577, still sent by current clients (D21)
use rmcp::model::Root;
use rmcp::service::{NotificationContext, Peer, RoleServer};
use rmcp::{tool, tool_handler, tool_router, ServerHandler, ServiceExt};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use ci_core::config::{Config, StoragePaths};
use ci_core::telemetry::TelemetrySink;
use ci_llm::{AgentWorkPackage, RoleRouter};

use crate::handlers::{ContextHandler, HandlerReply, ResolvedProject};
use crate::tools::*;

/// Single global MCP server serving multiple projects with dynamic root resolution (D21).
#[derive(Clone)]
pub struct ContextInitServer {
    pub(crate) default_root: Option<PathBuf>,
    pub(crate) roots: Arc<RwLock<Vec<PathBuf>>>,
    pub(crate) work_packages: Arc<RwLock<HashMap<String, AgentWorkPackage>>>,
    pub(crate) telemetry_sink: Option<Arc<dyn TelemetrySink>>,
}

impl ContextInitServer {
    /// Creates a new `ContextInitServer` with an optional fallback project root.
    pub fn new(default_root: Option<PathBuf>) -> Self {
        Self {
            default_root: default_root.map(|p| normalize_existing_path(&p)),
            roots: Arc::new(RwLock::new(Vec::new())),
            work_packages: Arc::new(RwLock::new(HashMap::new())),
            telemetry_sink: None,
        }
    }

    /// Injects an explicit telemetry sink for recording feedback and operational events.
    pub fn with_telemetry_sink(mut self, sink: Arc<dyn TelemetrySink>) -> Self {
        self.telemetry_sink = Some(sink);
        self
    }

    /// Adds a project root to the active MCP client roots list.
    pub async fn add_root(&self, root: PathBuf) {
        let norm = normalize_existing_path(&root);
        let mut w = self.roots.write().await;
        if !w.contains(&norm) {
            w.push(norm);
        }
    }

    /// Replaces the active MCP client roots list.
    pub async fn set_roots(&self, roots: Vec<PathBuf>) {
        let norm = roots
            .into_iter()
            .map(|p| normalize_existing_path(&p))
            .collect();
        let mut w = self.roots.write().await;
        *w = norm;
    }

    /// Returns a copy of the active MCP client roots list.
    pub async fn get_roots(&self) -> Vec<PathBuf> {
        self.roots.read().await.clone()
    }

    /// Registers an `AgentWorkPackage` for subsequent validation in `context_submit` (D14).
    pub async fn register_work_package(&self, pkg: AgentWorkPackage) {
        let mut w = self.work_packages.write().await;
        w.insert(pkg.package_id.clone(), pkg);
    }

    /// Resolves the project root directory following the 3-tier cascade (D21, R3):
    /// 1. Tool argument `path` or `project` (if passed)
    /// 2. MCP client roots from `roots/list` (if registered)
    /// 3. Server `default_root` or process current working directory (`cwd`)
    pub async fn resolve_project_root(
        &self,
        arg_path: Option<&str>,
        arg_project: Option<&str>,
    ) -> PathBuf {
        // Cascade Step 1: Explicit tool argument `project` or `path`
        let candidate_arg = arg_project
            .filter(|s| !s.trim().is_empty())
            .or_else(|| arg_path.filter(|s| !s.trim().is_empty()));

        if let Some(name) = arg_project
            .map(str::trim)
            .filter(|n| !n.is_empty() && *n != "." && *n != ".." && !n.contains(['/', '\\']))
        {
            if let Some(child) = self.child_repo(name).await {
                return child;
            }
        }

        if let Some(target) = candidate_arg {
            let p = PathBuf::from(target.trim());
            let abs_candidate = if p.is_absolute() {
                p
            } else if let Ok(cwd) = std::env::current_dir() {
                cwd.join(p)
            } else {
                p
            };

            if let Some(root) = find_project_root_upward(&abs_candidate) {
                return root;
            }
            if abs_candidate.exists() {
                return normalize_existing_path(&abs_candidate);
            }
        }

        // Cascade Step 2: MCP roots/list registered by client
        let registered_roots = self.roots.read().await;
        if let Some(first_root) = registered_roots.first() {
            if let Some(root) = find_project_root_upward(first_root) {
                return root;
            }
            return first_root.clone();
        }
        drop(registered_roots);

        // Cascade Step 3: Default root or current working directory
        if let Some(ref def) = self.default_root {
            if let Some(root) = find_project_root_upward(def) {
                return root;
            }
            return def.clone();
        }

        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        if let Some(root) = find_project_root_upward(&cwd) {
            return root;
        }
        normalize_existing_path(&cwd)
    }

    /// `<workspace>/<name>` for a bare repository name, looked up under the registered roots,
    /// and the default root (D6); never above them.
    async fn child_repo(&self, name: &str) -> Option<PathBuf> {
        let mut bases: Vec<PathBuf> = self.roots.read().await.clone();
        bases.extend(self.default_root.clone());
        bases
            .into_iter()
            .map(|b| b.join(name))
            .find(|c| c.is_dir())
            .map(|c| find_project_root_upward(&c).unwrap_or_else(|| normalize_existing_path(&c)))
    }

    /// Builds the model router for a project's configuration (D16).
    fn router_for(&self, project: &ResolvedProject) -> Result<RoleRouter, rmcp::ErrorData> {
        RoleRouter::new(project.config.clone(), self.telemetry_sink.clone())
            .map(|r| r.with_project_id(project.project_id.clone()))
            .map_err(|e| rmcp::ErrorData::invalid_params(format!("{e:#}"), None))
    }

    /// Registers a pending agent work package (if any) and returns the reply text.
    async fn accept_reply(&self, reply: HandlerReply) -> String {
        if let Some(pkg) = reply.pending_package {
            self.register_work_package(pkg).await;
        }
        reply.text
    }

    /// Resolves the full `ResolvedProject` context for the requested arguments.
    pub async fn resolve_context(
        &self,
        arg_path: Option<&str>,
        arg_project: Option<&str>,
    ) -> ResolvedProject {
        let root = self.resolve_project_root(arg_path, arg_project).await;
        ResolvedProject::load(root.clone()).unwrap_or_else(|_| ResolvedProject {
            root: root.clone(),
            config: Config::default(),
            project_id: root
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "project".to_string()),
            storage_paths: StoragePaths::resolve(Some(&root)),
            knowledge_dir: root.join(ci_core::analysis::REPO_KNOWLEDGE_DIR),
            storage: ci_core::config::StorageMode::Repo,
        })
    }
}

/// Climbs directory tree upward from `start` to locate the project root marker.
///
/// Distinguishes primary root markers (such as `.context-init.toml`, `.git`, existing knowledge base,
/// and workspace roots) from secondary package manifest markers to correctly resolve monorepos
/// and Cargo workspaces without getting stuck on nested member crates.
pub fn find_project_root_upward(start: &Path) -> Option<PathBuf> {
    let canonical = fs::canonicalize(start).unwrap_or_else(|_| start.to_path_buf());
    let mut primary_candidate: Option<PathBuf> = None;
    let mut secondary_candidate: Option<PathBuf> = None;

    let mut curr = Some(canonical.as_path());
    while let Some(dir) = curr {
        let is_primary = dir.join(".context-init.toml").is_file()
            || dir
                .join(ci_core::analysis::REPO_KNOWLEDGE_DIR)
                .join("L0_index.json")
                .is_file()
            || dir.join(".git").exists()
            || dir.join("pnpm-workspace.yaml").is_file()
            || is_cargo_workspace(dir);

        if is_primary {
            if primary_candidate.is_none() {
                primary_candidate = Some(dir.to_path_buf());
            }
        } else if secondary_candidate.is_none() {
            let is_secondary = dir.join("Cargo.toml").is_file()
                || dir.join("package.json").is_file()
                || dir.join("pyproject.toml").is_file()
                || dir.join("go.mod").is_file();
            if is_secondary {
                secondary_candidate = Some(dir.to_path_buf());
            }
        }

        curr = dir.parent();
    }

    primary_candidate.or(secondary_candidate)
}

fn is_cargo_workspace(dir: &Path) -> bool {
    let cargo_toml = dir.join("Cargo.toml");
    if let Ok(content) = fs::read_to_string(&cargo_toml) {
        content.contains("[workspace]")
    } else {
        false
    }
}

fn normalize_existing_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[tool_router]
impl ContextInitServer {
    /// Check knowledge base freshness against disk files and Git status.
    #[tool(
        name = "context_check",
        description = "Check knowledge base freshness against disk files and Git status."
    )]
    pub async fn check_drift(
        &self,
        params: Parameters<McpCheckParams>,
    ) -> Result<String, rmcp::ErrorData> {
        let p = params.0;
        let resolved = self
            .resolve_context(p.path.as_deref(), p.project.as_deref())
            .await;
        ContextHandler::check_drift(&resolved, p.cursor.as_deref())
            .map_err(|e| rmcp::ErrorData::internal_error(format!("{:#}", e), None))
    }

    /// List all project subsystems and metrics from L0 index.
    #[tool(
        name = "context_get_subsystems",
        description = "List all project subsystems and metrics from L0 index."
    )]
    pub async fn get_subsystems(
        &self,
        params: Parameters<McpGetSubsystemsParams>,
    ) -> Result<String, rmcp::ErrorData> {
        let p = params.0;
        let resolved = self
            .resolve_context(p.path.as_deref(), p.project.as_deref())
            .await;
        ContextHandler::get_subsystems(&resolved, p.cursor.as_deref())
            .map_err(|e| rmcp::ErrorData::internal_error(format!("{:#}", e), None))
    }

    /// Get L1 overview card for a specific subsystem by ID.
    #[tool(
        name = "context_get_subsystem",
        description = "Get L1 overview card for a specific subsystem by ID."
    )]
    pub async fn get_subsystem(
        &self,
        params: Parameters<McpGetSubsystemParams>,
    ) -> Result<String, rmcp::ErrorData> {
        let p = params.0;
        let resolved = self
            .resolve_context(p.path.as_deref(), p.project.as_deref())
            .await;
        ContextHandler::get_subsystem(&resolved, &p.subsystem_id, p.cursor.as_deref())
            .map_err(|e| rmcp::ErrorData::internal_error(format!("{:#}", e), None))
    }

    /// Pack subsystem code into AST-compressed context for LLM analysis.
    #[tool(
        name = "context_pack",
        description = "Pack subsystem code into AST-compressed context for LLM analysis."
    )]
    pub async fn pack(&self, params: Parameters<McpPackParams>) -> Result<String, rmcp::ErrorData> {
        let p = params.0;
        let resolved = self
            .resolve_context(p.path.as_deref(), p.project.as_deref())
            .await;
        ContextHandler::pack(
            &resolved,
            &p.subsystem_id,
            p.compress.unwrap_or(true),
            p.style.as_deref().unwrap_or("xml"),
            p.cursor.as_deref(),
        )
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{:#}", e), None))
    }

    /// Submit agent work package results for schema validation and integration.
    #[tool(
        name = "context_submit",
        description = "Submit agent work package results for schema validation and integration."
    )]
    pub async fn submit(
        &self,
        params: Parameters<McpSubmitParams>,
    ) -> Result<String, rmcp::ErrorData> {
        let p = params.0;
        let pkg = self.work_packages.read().await.get(&p.package_id).cloned();
        let resolved = self
            .resolve_context(p.path.as_deref(), p.project.as_deref())
            .await;
        ContextHandler::submit(&resolved, &p.package_id, pkg.as_ref(), &p.submission)
            .map_err(|e| rmcp::ErrorData::internal_error(format!("{:#}", e), None))
    }

    /// Record agent feedback signal on tool utility and freshness.
    #[tool(
        name = "context_feedback",
        description = "Record agent feedback signal on tool utility and freshness."
    )]
    pub async fn feedback(
        &self,
        params: Parameters<McpFeedbackParams>,
    ) -> Result<String, rmcp::ErrorData> {
        let p = params.0;
        let resolved = self
            .resolve_context(p.path.as_deref(), p.project.as_deref())
            .await;
        ContextHandler::feedback(
            &resolved,
            &p.tool,
            &p.verdict,
            p.note.as_deref(),
            self.telemetry_sink.as_deref(),
        )
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{:#}", e), None))
    }

    /// Initialize OpenViking L0-L2 knowledge base for a project.
    #[tool(
        name = "context_init",
        description = "Initialize OpenViking L0-L2 knowledge base for a project."
    )]
    pub async fn init(&self, params: Parameters<McpInitParams>) -> Result<String, rmcp::ErrorData> {
        let p = params.0;
        let resolved = self
            .resolve_context(p.path.as_deref(), p.project.as_deref())
            .await;
        let router = self.router_for(&resolved)?;
        let reply = ContextHandler::init(&resolved, &p, &router)
            .map_err(|e| rmcp::ErrorData::internal_error(format!("{:#}", e), None))?;
        Ok(self.accept_reply(reply).await)
    }

    /// Incrementally update knowledge base when architectural drift is detected.
    #[tool(
        name = "context_update",
        description = "Incrementally update knowledge base when architectural drift is detected."
    )]
    pub async fn update(
        &self,
        params: Parameters<McpUpdateParams>,
    ) -> Result<String, rmcp::ErrorData> {
        let p = params.0;
        let resolved = self
            .resolve_context(p.path.as_deref(), p.project.as_deref())
            .await;
        let router = self.router_for(&resolved)?;
        let reply = ContextHandler::update(&resolved, &p, &router)
            .map_err(|e| rmcp::ErrorData::internal_error(format!("{:#}", e), None))?;
        Ok(self.accept_reply(reply).await)
    }

    /// Map of the Git repositories inside a workspace directory.
    #[tool(
        name = "context_workspace",
        description = "List the Git repositories of a multi-repo workspace with branches, dirty counts and cross-project shared symbols."
    )]
    pub async fn workspace(
        &self,
        params: Parameters<McpWorkspaceParams>,
    ) -> Result<String, rmcp::ErrorData> {
        let p = params.0;
        let resolved = self.resolve_context(p.path.as_deref(), None).await;
        // An explicit directory is the workspace itself; otherwise use the resolved project root.
        let root = p
            .path
            .as_deref()
            .map(PathBuf::from)
            .filter(|d| d.is_dir())
            .map(|d| normalize_existing_path(&d))
            .unwrap_or_else(|| resolved.root.clone());
        ContextHandler::workspace(
            &root,
            resolved.config.max_response_tokens,
            p.cursor.as_deref(),
        )
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{:#}", e), None))
    }

    /// Scan project topology, directories, and file extensions without LLM.
    #[tool(
        name = "context_scan",
        description = "Scan project topology, directories, and file extensions without LLM."
    )]
    pub async fn scan(&self, params: Parameters<McpScanParams>) -> Result<String, rmcp::ErrorData> {
        let p = params.0;
        let resolved = self
            .resolve_context(p.path.as_deref(), p.project.as_deref())
            .await;
        ContextHandler::scan(&resolved, p.cursor.as_deref())
            .map_err(|e| rmcp::ErrorData::internal_error(format!("{:#}", e), None))
    }

    /// Generate task digest and modified subsystem context from Git diff.
    #[tool(
        name = "context_task",
        description = "Generate task digest and modified subsystem context from Git diff."
    )]
    pub async fn task(&self, params: Parameters<McpTaskParams>) -> Result<String, rmcp::ErrorData> {
        let p = params.0;
        let arg_path = p.path.as_deref().or(p.repo.as_deref());
        let arg_proj = p.project.as_deref();
        let resolved = self.resolve_context(arg_path, arg_proj).await;
        ContextHandler::task(
            &resolved,
            p.paths.as_deref(),
            p.base_ref.as_deref(),
            p.target.as_deref(),
            p.cursor.as_deref(),
        )
        .map_err(|e| rmcp::ErrorData::internal_error(format!("{:#}", e), None))
    }

    /// Search subsystems, cards, and symbols by query.
    #[tool(
        name = "context_search",
        description = "Search subsystems, cards, and symbols by query."
    )]
    pub async fn search(
        &self,
        params: Parameters<McpSearchParams>,
    ) -> Result<String, rmcp::ErrorData> {
        let p = params.0;
        let resolved = self
            .resolve_context(p.path.as_deref(), p.project.as_deref())
            .await;
        ContextHandler::search(&resolved, &p.query, p.cursor.as_deref())
            .map_err(|e| rmcp::ErrorData::internal_error(format!("{:#}", e), None))
    }
}

#[tool_handler(router = Self::tool_router())]
impl ServerHandler for ContextInitServer {
    async fn on_initialized(&self, context: NotificationContext<RoleServer>) {
        self.refresh_roots(&context.peer).await;
    }

    async fn on_roots_list_changed(&self, context: NotificationContext<RoleServer>) {
        self.refresh_roots(&context.peer).await;
    }
}

impl ContextInitServer {
    /// Fetches `roots/list` from the client (if it declared the `roots` capability) and
    /// stores the result as cascade step 2 (D21). Failures are logged and ignored.
    #[allow(deprecated)]
    pub async fn refresh_roots(&self, peer: &Peer<RoleServer>) {
        let supports_roots = peer
            .peer_info()
            .is_some_and(|info| info.capabilities.roots.is_some());
        if !supports_roots {
            return;
        }
        match peer.list_roots().await {
            Ok(result) => self.set_roots(roots_to_paths(&result.roots)).await,
            Err(e) => eprintln!("context-init: roots/list failed: {e}"),
        }
    }
}

/// Converts MCP roots to filesystem paths, keeping only `file://` URIs.
#[allow(deprecated)]
pub fn roots_to_paths(roots: &[Root]) -> Vec<PathBuf> {
    roots
        .iter()
        .filter_map(|r| r.uri.strip_prefix("file://"))
        .map(|rest| {
            let decoded = percent_decode(rest);
            // `file:///C:/x` → `C:/x` on Windows
            if cfg!(windows) {
                PathBuf::from(decoded.trim_start_matches('/'))
            } else {
                PathBuf::from(decoded)
            }
        })
        .collect()
}

fn percent_decode(s: &str) -> String {
    let hex = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 3 <= bytes.len() {
            if let (Some(hi), Some(lo)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(hi << 4 | lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Runs the MCP server listening on stdio transport (D21).
pub async fn run_mcp_server(default_root: Option<&Path>) -> Result<()> {
    eprintln!(
        "context-init MCP server starting (stdio), default_root: {:?}",
        default_root
    );

    let server = ContextInitServer::new(default_root.map(|p| p.to_path_buf()));
    let (stdin, stdout) = rmcp::transport::io::stdio();

    let running = server
        .serve((stdin, stdout))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to initialize MCP server: {:?}", e))?;

    eprintln!("context-init MCP server listening on stdio.");
    let _ = running.waiting().await;
    eprintln!("context-init MCP server stopped.");

    Ok(())
}
