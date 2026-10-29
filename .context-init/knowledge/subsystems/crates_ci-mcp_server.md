---
directory: crates/ci-mcp/src/server.rs
source:
  kind: git
  blob_sha: 6bf6ccd7e20a1a9b83b76c2d581bf6ff1976e7d6
  commit_sha: 4f153c6ee310b681808626eb7348ac3ca484e21d
generated_by:
  component: context-init
  role: init
  model: none
freshness:
  total_entries: 1
  sampled_entries: 1
  unsampled_entries: 0
  pending_child_changes: 0
---

# Подсистема: Ci Mcp Server (L1)

Модуль-файл `crates/ci-mcp/src/server.rs`

> **Идентификатор:** `crates_ci-mcp_server`
> **Теги:** Rust (Cargo), crates_ci-mcp_server

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-mcp/src/server.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub struct ContextInitServer
pub fn new(default_root: Option<PathBuf>) -> Self
pub fn with_telemetry_sink(mut self, sink: Arc<dyn TelemetrySink>) -> Self
pub async fn add_root(&self, root: PathBuf)
pub async fn set_roots(&self, roots: Vec<PathBuf>)
pub async fn get_roots(&self) -> Vec<PathBuf>
pub async fn register_work_package(&self, pkg: AgentWorkPackage)
pub async fn resolve_project_root(&self, arg_path: Option<&str>, arg_project: Option<&str>, ) -> PathBuf
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
