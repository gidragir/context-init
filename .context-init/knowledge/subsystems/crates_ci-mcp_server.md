---
directory: crates/ci-mcp/src/server.rs
source:
  kind: git
  blob_sha: 7fde93153911b2504262005edfd994e16434685d
  commit_sha: fff7dd106863757218c552cb8ef5b18e46c70899
generated_by:
  component: context-init
  role: update
  model: none
freshness:
  total_entries: 1
  sampled_entries: 1
  unsampled_entries: 0
  pending_child_changes: 0
---

# Подсистема: MCP-сервер (L1)

ContextInitServer на stdio: регистрация инструментов, MCP roots (roots/list) для выбора проекта; TelemetrySink подключается через with_telemetry_sink (в продакшен-запуске run_mcp_server не подключён).

> **Идентификатор:** `crates_ci-mcp_server`
> **Теги:** mcp, roots, server, telemetry

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-mcp/src/server.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub struct ContextInitServer
pub fn new(default_root: Option<PathBuf>) -> Self
pub fn stale_binary_footer(&self) -> Option<&'static str>
pub fn with_telemetry_sink(mut self, sink: Arc<dyn TelemetrySink>) -> Self
pub async fn add_root(&self, root: PathBuf)
pub async fn set_roots(&self, roots: Vec<PathBuf>)
pub async fn get_roots(&self) -> Vec<PathBuf>
pub async fn register_work_package(&self, pkg: AgentWorkPackage)
pub async fn resolve_project_root(&self, arg_path: Option<&str>, arg_project: Option<&str>, ) -> PathBuf
pub fn call_context(&self) -> CallContext
pub(crate) async fn instrumented<F, Fut>(&self, tool: &'static str, recorder: Recorder, f: F, ) -> Result<String, rmcp :: ErrorData> where F: FnOnce() -> Fut, Fut: Future<Output = anyhow :: Result<String>>,
pub async fn resolve_context(&self, arg_path: Option<&str>, arg_project: Option<&str>, ) -> ResolvedProject
pub fn find_project_root_upward(start: &Path) -> Option<PathBuf>
pub async fn check_drift(&self, params: Parameters<McpCheckParams>, ) -> Result<String, rmcp :: ErrorData>
pub async fn get_subsystems(&self, params: Parameters<McpGetSubsystemsParams>, ) -> Result<String, rmcp :: ErrorData>
pub async fn get_subsystem(&self, params: Parameters<McpGetSubsystemParams>, ) -> Result<String, rmcp :: ErrorData>
pub async fn pack(&self, params: Parameters<McpPackParams>) -> Result<String, rmcp :: ErrorData>
pub async fn submit(&self, params: Parameters<McpSubmitParams>, ) -> Result<String, rmcp :: ErrorData>
pub async fn feedback(&self, params: Parameters<McpFeedbackParams>, ) -> Result<String, rmcp :: ErrorData>
pub async fn init(&self, params: Parameters<McpInitParams>) -> Result<String, rmcp :: ErrorData>
pub async fn update(&self, params: Parameters<McpUpdateParams>, ) -> Result<String, rmcp :: ErrorData>
pub async fn workspace(&self, params: Parameters<McpWorkspaceParams>, ) -> Result<String, rmcp :: ErrorData>
pub async fn scan(&self, params: Parameters<McpScanParams>) -> Result<String, rmcp :: ErrorData>
pub async fn task(&self, params: Parameters<McpTaskParams>) -> Result<String, rmcp :: ErrorData>
pub async fn search(&self, params: Parameters<McpSearchParams>, ) -> Result<String, rmcp :: ErrorData>
pub async fn refresh_roots(&self, peer: &Peer<RoleServer>)
pub fn roots_to_paths(roots: &[Root]) -> Vec<PathBuf>
pub async fn run_mcp_server(default_root: Option<&Path>) -> Result<()>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-mcp
```
