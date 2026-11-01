---
directory: crates/ci-mcp/src/handlers.rs
source:
  kind: git
  blob_sha: f5dac95b6e0f0dedfad7e67bae47b9a196d39357
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

# Подсистема: Обработчики MCP (L1)

ResolvedProject (загрузка конфига и базы), обработчики get_subsystems/get_subsystem/pack/search/task/check/submit; ограничение pack all (5 страниц), сохранение агентских submission с проверкой id.

> **Идентификатор:** `crates_ci-mcp_handlers`
> **Теги:** handlers, mcp, submit

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
pub fn recorder(&self, sink: Option<Arc<dyn TelemetrySink>>) -> Recorder
pub fn knowledge_dir_for(&self, mode: StorageMode) -> PathBuf
pub fn load_l0_index(&self) -> Result<L0Index>
pub fn card_file(&self, subsystem: &SubsystemInfo) -> Option<PathBuf>
pub fn check_subsystem_staleness(&self, subsystem: &SubsystemInfo) -> (bool, Option<String>)
pub const DEFAULT_SEARCH_TOP_K: usize = 10
pub struct HandlerReply
pub struct SubmitReply
pub struct ContextHandler
pub fn check_drift(project: &ResolvedProject, cursor: Option<&str>) -> Result<String>
pub fn get_subsystems(project: &ResolvedProject, cursor: Option<&str>, tel: &TelemetryScope, ) -> Result<String>
pub fn get_subsystem(project: &ResolvedProject, subsystem_id: &str, cursor: Option<&str>, tel: &TelemetryScope, ) -> Result<String>
pub fn pack(project: &ResolvedProject, subsystem_id: &str, compress: bool, style: &str, cursor: Option<&str>, tel: &TelemetryScope, ) -> Result<String>
pub fn submit(project: &ResolvedProject, package_id: &str, work_package: Option<&AgentWorkPackage>, submission: &serde_json :: Value, ) -> Result<SubmitReply>
pub fn feedback(project: &ResolvedProject, tool: &str, verdict: &str, note: Option<&str>, telemetry_sink: Option<&dyn TelemetrySink>, ) -> Result<String>
pub fn scan(project: &ResolvedProject, cursor: Option<&str>) -> Result<String>
pub fn workspace(root: &Path, max_tokens: usize, cursor: Option<&str>) -> Result<String>
pub fn init(project: &ResolvedProject, params: &McpInitParams, router: &RoleRouter, ) -> Result<HandlerReply>
pub fn update(project: &ResolvedProject, params: &McpUpdateParams, router: &RoleRouter, ) -> Result<HandlerReply>
pub fn task(project: &ResolvedProject, paths: Option<&[String]>, base_ref: Option<&str>, target: Option<&str>, cursor: Option<&str>, tel: &TelemetryScope, ) -> Result<String>
pub fn search(project: &ResolvedProject, query: &str, cursor: Option<&str>, router: &RoleRouter, tel: &TelemetryScope, ) -> Result<String>
pub fn search_top_k(project: &ResolvedProject, query: &str, cursor: Option<&str>, router: &RoleRouter, top_k: usize, tel: &TelemetryScope, ) -> Result<String>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-mcp
```
