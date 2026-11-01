//! Single global MCP server, cascade project root resolution, and stdio runner (D20, D21, R3, R5).

use anyhow::Result;
use rmcp::handler::server::wrapper::Parameters;
#[allow(deprecated)] // MCP roots: deprecated by SEP-2577, still sent by current clients (D21)
use rmcp::model::Root;
use rmcp::service::{NotificationContext, Peer, RoleServer};
use rmcp::{tool, tool_handler, tool_router, ServerHandler, ServiceExt};
use std::collections::HashMap;
use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Instant, SystemTime};
use tokio::sync::RwLock;

use ci_core::config::{Config, StoragePaths};
use ci_core::telemetry::{
    classify_error, harness_from_client_name, CallContext, CallOutcome, Channel, ErrorClass,
    HarnessId, JsonlTelemetrySink, Recorder, TelemetryScope, TelemetrySink,
};
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
    /// Harness of the connected client, normalized from `clientInfo.name` on `initialized`.
    pub(crate) client_harness: Arc<std::sync::RwLock<HarnessId>>,
    /// Modification time of the running executable when the server started; `None` when
    /// it could not be read (no stale-binary detection then).
    pub(crate) exe_mtime_at_start: Option<SystemTime>,
    /// Path of the running executable captured at start. Kept because after an atomic
    /// `mv` install Linux reports `current_exe()` as `<path> (deleted)`.
    pub(crate) exe_path: Option<PathBuf>,
}

/// Appended to responses once the executable on disk is newer than the running process.
const STALE_BINARY_FOOTER: &str = "⚠ context-init обновлён на диске — перезапустите MCP-сервер";

/// Path of the running executable, without the `" (deleted)"` suffix Linux appends once the
/// file was replaced or removed.
fn current_exe_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let text = exe.to_string_lossy();
    Some(match text.strip_suffix(" (deleted)") {
        Some(stripped) => PathBuf::from(stripped),
        None => exe,
    })
}

fn file_mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).ok()?.modified().ok()
}

impl ContextInitServer {
    /// Creates a new `ContextInitServer` with an optional fallback project root.
    pub fn new(default_root: Option<PathBuf>) -> Self {
        let exe_path = current_exe_path();
        Self {
            default_root: default_root.map(|p| normalize_existing_path(&p)),
            roots: Arc::new(RwLock::new(Vec::new())),
            work_packages: Arc::new(RwLock::new(HashMap::new())),
            telemetry_sink: None,
            client_harness: Arc::new(std::sync::RwLock::new(HarnessId::Other)),
            exe_mtime_at_start: exe_path.as_deref().and_then(file_mtime),
            exe_path,
        }
    }

    /// Footer warning that the binary on disk changed since this server started (so the
    /// harness is still talking to an outdated process); `None` while it is unchanged.
    ///
    /// Compares the mtime of the path remembered at start with its mtime now. A path that
    /// is gone (replaced and not yet recreated, or removed) counts as changed, as long as
    /// the start mtime is known; without a known start mtime nothing can be detected.
    pub fn stale_binary_footer(&self) -> Option<&'static str> {
        let at_start = self.exe_mtime_at_start?;
        let now = self.exe_path.as_deref().and_then(file_mtime);
        (now != Some(at_start)).then_some(STALE_BINARY_FOOTER)
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
        let candidate_arg = arg_project
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .or_else(|| arg_path.map(str::trim).filter(|s| !s.is_empty()));

        if let Some(target) = candidate_arg {
            let p = PathBuf::from(target);
            if p.is_absolute() {
                if p.exists() {
                    if let Some(root) = find_project_root_upward(&p) {
                        return root;
                    }
                    return normalize_existing_path(&p);
                }
            } else {
                // 1. Try child_repo (which checks roots, default_root, sibling to default_root, and workspace_roots)
                if let Some(child) = self.child_repo(target).await {
                    return child;
                }

                // 2. Try cwd.join(p) only if it actually exists
                let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                let cwd_cand = cwd.join(&p);
                if cwd_cand.exists() {
                    if let Some(root) = find_project_root_upward(&cwd_cand) {
                        return root;
                    }
                    return normalize_existing_path(&cwd_cand);
                }

                // 3. Try sibling of cwd
                if let Some(parent) = cwd.parent() {
                    let sib = parent.join(&p);
                    if sib.exists() {
                        if let Some(root) = find_project_root_upward(&sib) {
                            return root;
                        }
                        return normalize_existing_path(&sib);
                    }
                }

                // 4. Try workspace_roots
                for ws in &Config::default().workspace_roots {
                    let cand = ws.join(&p);
                    if cand.exists() {
                        if let Some(root) = find_project_root_upward(&cand) {
                            return root;
                        }
                        return normalize_existing_path(&cand);
                    }
                }

                return cwd_cand;
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
    /// the default root, its siblings, and configured workspace roots; never above them.
    async fn child_repo(&self, name: &str) -> Option<PathBuf> {
        let mut bases: Vec<PathBuf> = self.roots.read().await.clone();
        bases.extend(self.default_root.clone());
        if let Some(def) = &self.default_root {
            if let Some(parent) = def.parent() {
                bases.push(parent.to_path_buf());
            }
        }
        bases.extend(Config::default().workspace_roots);
        bases
            .into_iter()
            .map(|b| b.join(name))
            .find(|c| c.is_dir())
            .map(|c| find_project_root_upward(&c).unwrap_or_else(|| normalize_existing_path(&c)))
    }

    /// Builds the model router for a project's configuration (D16).
    /// The error wraps an `invalid_params` `rmcp::ErrorData`, preserved by [`Self::instrumented`].
    fn router_for(&self, project: &ResolvedProject) -> anyhow::Result<RoleRouter> {
        RoleRouter::new(project.config.clone(), self.telemetry_sink.clone())
            .map(|r| r.with_project_id(project.project_id.clone()))
            .map_err(|e| {
                anyhow::Error::new(rmcp::ErrorData::invalid_params(format!("{e:#}"), None))
            })
    }

    /// Per-call telemetry context: MCP channel, current client harness, not nested.
    pub fn call_context(&self) -> CallContext {
        CallContext {
            harness: self
                .client_harness
                .read()
                .map(|h| *h)
                .unwrap_or(HarnessId::Other),
            channel: Channel::Mcp,
            nested: false,
        }
    }

    /// Telemetry scope handed to handlers: the project's recorder plus the current call context.
    fn scope(&self, recorder: &Recorder) -> TelemetryScope {
        TelemetryScope {
            recorder: recorder.clone(),
            ctx: self.call_context(),
        }
    }

    /// Runs a tool body, records one operational event through `recorder` (a no-op recorder
    /// when the resolved project has telemetry disabled) and maps errors to MCP errors.
    pub(crate) async fn instrumented<F, Fut>(
        &self,
        tool: &'static str,
        recorder: Recorder,
        f: F,
    ) -> Result<String, rmcp::ErrorData>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = anyhow::Result<String>>,
    {
        let ctx = self.call_context();
        let started = Instant::now();
        match f().await {
            Ok(text) => {
                recorder.record_call(&ctx, tool, started, CallOutcome::Ok);
                Ok(match self.stale_binary_footer() {
                    Some(footer) => format!("{text}\n{footer}"),
                    None => text,
                })
            }
            Err(e) => {
                let (class, mut mcp_err) = match e.downcast_ref::<rmcp::ErrorData>() {
                    Some(data) => (ErrorClass::InvalidParams, data.clone()),
                    None => (
                        classify_error(&e),
                        rmcp::ErrorData::internal_error(format!("{:#}", e), None),
                    ),
                };
                if let Some(footer) = self.stale_binary_footer() {
                    mcp_err.message = format!("{}\n{footer}", mcp_err.message).into();
                }
                recorder.record_call(&ctx, tool, started, CallOutcome::Err(class));
                Err(mcp_err)
            }
        }
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
        ResolvedProject::load(root.clone()).unwrap_or_else(|e| {
            // stderr only: stdout carries the MCP protocol.
            eprintln!(
                "context-init: invalid config for {}, using defaults: {e:#}",
                root.display()
            );
            ResolvedProject {
                root: root.clone(),
                config: Config::default(),
                project_id: root
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "project".to_string()),
                storage_paths: StoragePaths::resolve(Some(&root)),
                knowledge_dir: root.join(ci_core::analysis::REPO_KNOWLEDGE_DIR),
                storage: ci_core::config::StorageMode::Repo,
            }
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

#[tool_router(vis = "pub(crate)")]
impl ContextInitServer {
    /// Check knowledge base freshness against disk files and Git status.
    #[tool(
        name = "context_check",
        description = "Check knowledge base freshness against disk files and Git status, reporting drift. Run before finishing a task that changed structure (added/moved modules). Cheap, compact report."
    )]
    pub async fn check_drift(
        &self,
        params: Parameters<McpCheckParams>,
    ) -> Result<String, rmcp::ErrorData> {
        let p = params.0;
        let resolved = self
            .resolve_context(p.path.as_deref(), p.project.as_deref())
            .await;
        let recorder = resolved.recorder(self.telemetry_sink.clone());
        self.instrumented("context_check", recorder, || async {
            ContextHandler::check_drift(&resolved, p.cursor.as_deref())
        })
        .await
    }

    /// List all project subsystems and metrics from L0 index.
    #[tool(
        name = "context_get_subsystems",
        description = "List all project subsystems and metrics from the L0 index. Use at the start of unfamiliar work to see the whole map. One compact page instead of exploring directories."
    )]
    pub async fn get_subsystems(
        &self,
        params: Parameters<McpGetSubsystemsParams>,
    ) -> Result<String, rmcp::ErrorData> {
        let p = params.0;
        let resolved = self
            .resolve_context(p.path.as_deref(), p.project.as_deref())
            .await;
        let recorder = resolved.recorder(self.telemetry_sink.clone());
        let tel = self.scope(&recorder);
        self.instrumented("context_get_subsystems", recorder, || async {
            ContextHandler::get_subsystems(&resolved, p.cursor.as_deref(), &tel)
        })
        .await
    }

    /// Get L1 overview card for a specific subsystem by ID.
    #[tool(
        name = "context_get_subsystem",
        description = "Get the L1 overview card (files, key symbols, entrypoints, tests) of one subsystem by ID. Use before opening files of one subsystem. A few hundred tokens instead of reading sources."
    )]
    pub async fn get_subsystem(
        &self,
        params: Parameters<McpGetSubsystemParams>,
    ) -> Result<String, rmcp::ErrorData> {
        let p = params.0;
        let resolved = self
            .resolve_context(p.path.as_deref(), p.project.as_deref())
            .await;
        let recorder = resolved.recorder(self.telemetry_sink.clone());
        let tel = self.scope(&recorder);
        self.instrumented("context_get_subsystem", recorder, || async {
            ContextHandler::get_subsystem(&resolved, &p.subsystem_id, p.cursor.as_deref(), &tel)
        })
        .await
    }

    /// Pack subsystem code into AST-compressed context for LLM analysis.
    #[tool(
        name = "context_pack",
        description = "Pack a subsystem's code into AST-compressed context. Use instead of reading 3+ files of one subsystem; bodies elided, signatures kept, far fewer tokens than raw files."
    )]
    pub async fn pack(&self, params: Parameters<McpPackParams>) -> Result<String, rmcp::ErrorData> {
        let p = params.0;
        let resolved = self
            .resolve_context(p.path.as_deref(), p.project.as_deref())
            .await;
        let recorder = resolved.recorder(self.telemetry_sink.clone());
        let tel = self.scope(&recorder);
        self.instrumented("context_pack", recorder, || async {
            ContextHandler::pack(
                &resolved,
                &p.subsystem_id,
                p.compress.unwrap_or(true),
                p.style.as_deref().unwrap_or("xml"),
                p.cursor.as_deref(),
                &tel,
            )
        })
        .await
    }

    /// Submit agent work package results for schema validation and integration.
    #[tool(
        name = "context_submit",
        description = "Submit agent work package results for schema validation and integration. Use after context_init/context_update returned a package_id and you produced the JSON answer."
    )]
    pub async fn submit(
        &self,
        params: Parameters<McpSubmitParams>,
    ) -> Result<String, rmcp::ErrorData> {
        let p = params.0;
        // Packages are single-use: taken out of the map for the duration of the call, so two
        // concurrent submissions cannot both apply it, and put back only when rejected so the
        // agent can retry.
        let pkg = self.work_packages.write().await.remove(&p.package_id);
        let resolved = self
            .resolve_context(p.path.as_deref(), p.project.as_deref())
            .await;
        let recorder = resolved.recorder(self.telemetry_sink.clone());
        let mut accepted = false;
        let accepted_flag = &mut accepted;
        let (resolved_ref, params_ref, pkg_ref) = (&resolved, &p, pkg.as_ref());
        let out = self
            .instrumented("context_submit", recorder, move || async move {
                let reply = ContextHandler::submit(
                    resolved_ref,
                    &params_ref.package_id,
                    pkg_ref,
                    &params_ref.submission,
                )?;
                *accepted_flag = reply.accepted;
                Ok(reply.text)
            })
            .await;
        if let (false, Some(pkg)) = (accepted, pkg) {
            self.work_packages
                .write()
                .await
                .insert(p.package_id.clone(), pkg);
        }
        out
    }

    /// Record agent feedback signal on tool utility and freshness.
    #[tool(
        name = "context_feedback",
        description = "Record agent feedback signal on tool utility and freshness. Use after a context_* answer was clearly helpful, wrong or stale; one short call."
    )]
    pub async fn feedback(
        &self,
        params: Parameters<McpFeedbackParams>,
    ) -> Result<String, rmcp::ErrorData> {
        let p = params.0;
        let resolved = self
            .resolve_context(p.path.as_deref(), p.project.as_deref())
            .await;
        let recorder = resolved.recorder(self.telemetry_sink.clone());
        self.instrumented("context_feedback", recorder, || async {
            ContextHandler::feedback(
                &resolved,
                &p.tool,
                &p.verdict,
                p.note.as_deref(),
                self.telemetry_sink.as_deref(),
            )
        })
        .await
    }

    /// Initialize OpenViking L0-L2 knowledge base for a project.
    #[tool(
        name = "context_init",
        description = "Initialize the OpenViking L0-L2 knowledge base for a project. Use once when a project has no knowledge base yet (other context_* tools report it missing)."
    )]
    pub async fn init(&self, params: Parameters<McpInitParams>) -> Result<String, rmcp::ErrorData> {
        let p = params.0;
        let resolved = self
            .resolve_context(p.path.as_deref(), p.project.as_deref())
            .await;
        let recorder = resolved.recorder(self.telemetry_sink.clone());
        self.instrumented("context_init", recorder, || async {
            let router = self.router_for(&resolved)?;
            let reply = ContextHandler::init(&resolved, &p, &router)?;
            Ok(self.accept_reply(reply).await)
        })
        .await
    }

    /// Incrementally update knowledge base when architectural drift is detected.
    #[tool(
        name = "context_update",
        description = "Incrementally update the knowledge base after architectural drift. Use when context_check reports drift or stale cards; refreshes only changed subsystems."
    )]
    pub async fn update(
        &self,
        params: Parameters<McpUpdateParams>,
    ) -> Result<String, rmcp::ErrorData> {
        let p = params.0;
        let resolved = self
            .resolve_context(p.path.as_deref(), p.project.as_deref())
            .await;
        let recorder = resolved.recorder(self.telemetry_sink.clone());
        self.instrumented("context_update", recorder, || async {
            let router = self.router_for(&resolved)?;
            let reply = ContextHandler::update(&resolved, &p, &router)?;
            Ok(self.accept_reply(reply).await)
        })
        .await
    }

    /// Map of the Git repositories inside a workspace directory.
    #[tool(
        name = "context_workspace",
        description = "List the Git repositories of a multi-repo workspace with branches, dirty counts and cross-project shared symbols. Use when the root holds several repos, to pick one before other context_* calls."
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
        let recorder = resolved.recorder(self.telemetry_sink.clone());
        self.instrumented("context_workspace", recorder, || async {
            ContextHandler::workspace(
                &root,
                resolved.config.max_response_tokens,
                p.cursor.as_deref(),
            )
        })
        .await
    }

    /// Scan project topology, directories, and file extensions without LLM.
    #[tool(
        name = "context_scan",
        description = "Scan project topology, directories and file extensions without LLM. Use for a quick structural overview of a project that has no knowledge base yet."
    )]
    pub async fn scan(&self, params: Parameters<McpScanParams>) -> Result<String, rmcp::ErrorData> {
        let p = params.0;
        let resolved = self
            .resolve_context(p.path.as_deref(), p.project.as_deref())
            .await;
        let recorder = resolved.recorder(self.telemetry_sink.clone());
        self.instrumented("context_scan", recorder, || async {
            ContextHandler::scan(&resolved, p.cursor.as_deref())
        })
        .await
    }

    /// Generate task digest and modified subsystem context from Git diff.
    #[tool(
        name = "context_task",
        description = "Generate a task digest and the subsystems touched by a Git diff. Use with a diff/branch to see which subsystems a change touches before reviewing or extending it."
    )]
    pub async fn task(&self, params: Parameters<McpTaskParams>) -> Result<String, rmcp::ErrorData> {
        let p = params.0;
        let arg_path = p.path.as_deref().or(p.repo.as_deref());
        let arg_proj = p.project.as_deref();
        let resolved = self.resolve_context(arg_path, arg_proj).await;
        let recorder = resolved.recorder(self.telemetry_sink.clone());
        let tel = self.scope(&recorder);
        self.instrumented("context_task", recorder, || async {
            ContextHandler::task(
                &resolved,
                p.paths.as_deref(),
                p.base_ref.as_deref(),
                p.target.as_deref(),
                p.cursor.as_deref(),
                &tel,
            )
        })
        .await
    }

    /// Search subsystems, cards, and symbols by query.
    #[tool(
        name = "context_search",
        description = "Search subsystems, cards and symbols by query. Start here when you do not know which module handles a concept; returns ranked subsystems with signatures (~1-2k tokens) instead of reading files."
    )]
    pub async fn search(
        &self,
        params: Parameters<McpSearchParams>,
    ) -> Result<String, rmcp::ErrorData> {
        let p = params.0;
        let resolved = self
            .resolve_context(p.path.as_deref(), p.project.as_deref())
            .await;
        let recorder = resolved.recorder(self.telemetry_sink.clone());
        let tel = self.scope(&recorder);
        self.instrumented("context_search", recorder, || async {
            let router = self.router_for(&resolved)?;
            ContextHandler::search(&resolved, &p.query, p.cursor.as_deref(), &router, &tel)
        })
        .await
    }
}

#[tool_handler(router = Self::tool_router())]
impl ServerHandler for ContextInitServer {
    fn get_info(&self) -> rmcp::model::ServerConfig {
        let mut info = rmcp::model::ServerConfig::new(
            rmcp::model::ServerCapabilities::builder()
                .enable_tools()
                .build(),
        );
        info.server_info.version = format!("{}+{}", ci_core::VERSION, ci_core::BUILD);
        info
    }

    async fn on_initialized(&self, context: NotificationContext<RoleServer>) {
        let info = context.peer.peer_info();
        let harness = harness_from_client_name(info.as_ref().map(|i| i.client_info.name.as_str()));
        if let Ok(mut w) = self.client_harness.write() {
            *w = harness;
        }
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

    // Operational events are written per call only for projects whose own config opts in.
    let sink = JsonlTelemetrySink::new(StoragePaths::resolve(default_root).data_dir, true);
    let server = ContextInitServer::new(default_root.map(|p| p.to_path_buf()))
        .with_telemetry_sink(Arc::new(sink));
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
