---
directory: crates/ci-core/src/analysis/lang
source:
  kind: git
  blob_sha: 72d88de1b13cefcccff84698429bbc7ac2a01919
  commit_sha: ebcdb066e85279ae873a213c32310748678eefe2
generated_by:
  component: context-init
  role: update
  model: none
freshness:
  total_entries: 5
  sampled_entries: 5
  unsampled_entries: 0
  pending_child_changes: 0
---

# Подсистема: Ci Core Analysis Lang (L1)

Подсистема `crates/ci-core/src/analysis/lang` с ключевыми точками входа: crates/ci-core/src/analysis/lang/mod.rs

> **Идентификатор:** `crates_ci-core_analysis_lang`
> **Теги:** Rust (Cargo), crates_ci-core_analysis_lang

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/src/analysis/lang/bsl.rs` | Исходный код |
| `crates/ci-core/src/analysis/lang/mdo.rs` | Исходный код |
| `crates/ci-core/src/analysis/lang/mod.rs` | ⭐ Точка входа |
| `crates/ci-core/src/analysis/lang/python.rs` | Исходный код |
| `crates/ci-core/src/analysis/lang/rust.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub const ELIDED_BODY: &str = "⋮----"
pub trait LanguageAdapter
pub fn adapter_for_ext(ext: &str) -> Option<&'static dyn LanguageAdapter>
pub fn adapter_for_path(path: &Path) -> Option<&'static dyn LanguageAdapter>
pub struct BslAdapter
pub struct MdoObject
pub fn parse_mdo_xml(xml: &str) -> Result<MdoObject>
pub fn semantic_diff(old: Option<&MdoObject>, new: &MdoObject) -> Vec<String>
pub fn render_mdo_l2_card(obj: &MdoObject) -> String
pub(crate) fn discover_mdo_subsystems(root: &Path, files: &[String], ) -> (Vec<DiscoveredModule>, BTreeSet<String>)
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
