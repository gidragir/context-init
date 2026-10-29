---
directory: crates/ci-mcp/src/handlers.rs
source:
  kind: git
  blob_sha: 85b6840db601a513a10687b11f070d36da98f8a7
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

# Подсистема: Ci Mcp Handlers (L1)

Модуль-файл `crates/ci-mcp/src/handlers.rs`

> **Идентификатор:** `crates_ci-mcp_handlers`
> **Теги:** Rust (Cargo), crates_ci-mcp_handlers

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-mcp/src/handlers.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub const PACK_ALL_MAX_PAGES: usize = 5
pub struct ResolvedProject
pub fn load(root: PathBuf) -> Result<Self>
pub fn load_with(root: PathBuf, cli: &CliOverrides, extra_config: Option<&Path>, ) -> Result<Self>
pub fn knowledge_dir_for(&self, mode: StorageMode) -> PathBuf
pub fn load_l0_index(&self) -> Result<L0Index>
pub fn card_file(&self, subsystem: &SubsystemInfo) -> Option<PathBuf>
pub fn check_subsystem_staleness(&self, subsystem: &SubsystemInfo) -> (bool, Option<String>)
pub const DEFAULT_SEARCH_TOP_K: usize = 10
pub struct HandlerReply
pub struct ContextHandler
pub fn check_drift(project: &ResolvedProject, cursor: Option<&str>) -> Result<String>
pub fn get_subsystems(project: &ResolvedProject, cursor: Option<&str>) -> Result<String>
pub fn get_subsystem(project: &ResolvedProject, subsystem_id: &str, cursor: Option<&str>, ) -> Result<String>
pub fn pack(project: &ResolvedProject, subsystem_id: &str, compress: bool, style: &str, cursor: Option<&str>, ) -> Result<String>
pub fn submit(project: &ResolvedProject, package_id: &str, work_package: Option<&AgentWorkPackage>, submission: &serde_json :: Value, ) -> Result<String>
pub fn feedback(project: &ResolvedProject, tool: &str, verdict: &str, note: Option<&str>, telemetry_sink: Option<&dyn TelemetrySink>, ) -> Result<String>
pub fn scan(project: &ResolvedProject, cursor: Option<&str>) -> Result<String>
pub fn workspace(root: &Path, max_tokens: usize, cursor: Option<&str>) -> Result<String>
pub fn init(project: &ResolvedProject, params: &McpInitParams, router: &RoleRouter, ) -> Result<HandlerReply>
pub fn update(project: &ResolvedProject, params: &McpUpdateParams, router: &RoleRouter, ) -> Result<HandlerReply>
pub fn task(project: &ResolvedProject, paths: Option<&[String]>, base_ref: Option<&str>, target: Option<&str>, cursor: Option<&str>, ) -> Result<String>
pub fn search(project: &ResolvedProject, query: &str, cursor: Option<&str>, router: &RoleRouter, ) -> Result<String>
pub fn search_top_k(project: &ResolvedProject, query: &str, cursor: Option<&str>, router: &RoleRouter, top_k: usize, ) -> Result<String>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-mcp
```
