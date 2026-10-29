---
directory: crates/ci-core/src/analysis
source:
  kind: git
  blob_sha: 7d4efea0680f1542b17cfad068451159903b605c
  commit_sha: ebcdb066e85279ae873a213c32310748678eefe2
generated_by:
  component: context-init
  role: update
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
pub fn build_deterministic(analysis: &ProjectAnalysis) -> Vec<SubsystemInfo>
pub(crate) const GENERATED_SUMMARY_PREFIXES: [&str; 3] = ["Подсистема `", "Модуль и компоненты директории `", "Модуль-файл `", ]
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
