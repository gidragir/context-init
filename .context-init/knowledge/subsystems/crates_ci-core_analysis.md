---
directory: crates/ci-core/src/analysis
source:
  kind: git
  blob_sha: 90f12d766bf1a77aeb71c199587e4d029c4817b7
  commit_sha: 36198169b6c3e1dac832cf4204e7b96c6805df94
generated_by:
  component: context-init
  role: init
  model: none
freshness:
  total_entries: 10
  sampled_entries: 10
  unsampled_entries: 0
  pending_child_changes: 0
---

# Подсистема: Ci Core Analysis (L1)

Подсистема `crates/ci-core/src/analysis` с ключевыми точками входа: crates/ci-core/src/analysis/lang/mod.rs, crates/ci-core/src/analysis/mod.rs

> **Идентификатор:** `crates_ci-core_analysis`
> **Теги:** Rust (Cargo), crates_ci-core_analysis

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/src/analysis/lang/bsl.rs` | Исходный код |
| `crates/ci-core/src/analysis/lang/mdo.rs` | Исходный код |
| `crates/ci-core/src/analysis/lang/mod.rs` | ⭐ Точка входа |
| `crates/ci-core/src/analysis/lang/python.rs` | Исходный код |
| `crates/ci-core/src/analysis/lang/rust.rs` | Исходный код |
| `crates/ci-core/src/analysis/mod.rs` | ⭐ Точка входа |
| `crates/ci-core/src/analysis/scan.rs` | Исходный код |
| `crates/ci-core/src/analysis/scope.rs` | Исходный код |
| `crates/ci-core/src/analysis/subsystems.rs` | Исходный код |
| `crates/ci-core/src/analysis/types.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub const REPO_KNOWLEDGE_DIR: &str = ".context-init/knowledge"
pub fn resolve_knowledge_dir(root: &Path, mode: StorageMode, central_kb: &Path) -> PathBuf
pub struct BslAdapter
pub struct MdoObject
pub fn parse_mdo_xml(xml: &str) -> Result<MdoObject>
pub fn render_mdo_l2_card(obj: &MdoObject) -> String
pub(crate) fn discover_mdo_subsystems(root: &Path, files: &[String], ) -> (Vec<DiscoveredModule>, BTreeSet<String>)
pub fn find_object_card(root: &Path, scope: &ScopeConfig, id: &str) -> Result<Option<String>>
pub struct PythonAdapter
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
