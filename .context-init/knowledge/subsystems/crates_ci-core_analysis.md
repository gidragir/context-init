---
directory: crates/ci-core/src/analysis
source:
  kind: git
  blob_sha: 032f40c8d9ac8143b23008a6d914116c03138a60
  commit_sha: 4f153c6ee310b681808626eb7348ac3ca484e21d
generated_by:
  component: context-init
  role: init
  model: none
freshness:
  total_entries: 7
  sampled_entries: 7
  unsampled_entries: 0
  pending_child_changes: 0
---

# Подсистема: Ci Core Analysis (L1)

Подсистема `crates/ci-core/src/analysis` с ключевыми точками входа: crates/ci-core/src/analysis/mod.rs

> **Идентификатор:** `crates_ci-core_analysis`
> **Теги:** Rust (Cargo), crates_ci-core_analysis

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/src/analysis/mod.rs` | ⭐ Точка входа |
| `crates/ci-core/src/analysis/scan.rs` | Исходный код |
| `crates/ci-core/src/analysis/scope.rs` | Исходный код |
| `crates/ci-core/src/analysis/subsystems.rs` | Исходный код |
| `crates/ci-core/src/analysis/task.rs` | Исходный код |
| `crates/ci-core/src/analysis/types.rs` | Исходный код |
| `crates/ci-core/src/analysis/workspace.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub const REPO_KNOWLEDGE_DIR: &str = ".context-init/knowledge"
pub fn resolve_knowledge_dir(root: &Path, mode: StorageMode, central_kb: &Path) -> PathBuf
pub struct ScanStat
pub struct ScanReport
pub fn scan_project(root: &Path, scope: &ScopeConfig) -> Result<ScanReport>
pub fn render_scan_markdown(project_id: &str, root: &Path, report: &ScanReport) -> String
pub fn validate_scope(scope: &ScopeConfig) -> Result<()>
pub struct ScopeFilter
pub fn build(root: &Path, scope: &ScopeConfig) -> Result<Self>
pub fn files(&self) -> &[String]
pub fn allows_file(&self, path: &Path) -> bool
pub fn allows_dir(&self, path: &Path) -> bool
pub fn build_deterministic(analysis: &ProjectAnalysis) -> Vec<SubsystemInfo>
pub(crate) const GENERATED_SUMMARY_PREFIXES: [&str; 3] = ["Подсистема `", "Модуль и компоненты директории `", "Модуль-файл `", ]
pub fn subsystem_tokens(root_dir: &Path, sub: &SubsystemInfo) -> usize
pub struct TaskDigestRequest<'a>
pub fn build_task_digest(root: &Path, git: &dyn GitBackend, index: Option<&L0Index>, req: &TaskDigestRequest, ) -> Result<String>
pub struct DiscoveredModule
pub struct ProjectAnalysis
pub struct McpCheckResult
pub struct WorkspaceRepo
pub struct CrossLink
pub struct WorkspaceReport
pub fn analyze_workspace(workspace_root: &Path, git: &dyn GitBackend) -> Result<WorkspaceReport>
pub fn render_workspace_l0(report: &WorkspaceReport) -> String
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
