---
directory: crates/ci-core/src/analysis/lang
source:
  kind: git
  blob_sha: b401a4072acc3c805e3c84134342fd8c810a3c1f
  commit_sha: fff7dd106863757218c552cb8ef5b18e46c70899
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

# Подсистема: Языковые адаптеры (L1)

Трейт LanguageAdapter (language_id, extensions, extract_symbols) и адаптеры Rust, Python, BSL и MDO (1С); ELIDED_BODY — маркер вырезанного тела при сжатии.

> **Идентификатор:** `crates_ci-core_analysis_lang`
> **Теги:** 1c, ast, languages, python, rust

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
fn language_id(&self) -> &'static str
fn extensions(&self) -> &'static [&'static str]
fn extract_symbols(&self, source: &str) -> Vec<String>
fn compress_source(&self, source: &str) -> String
fn method_bodies(&self, source: &str) -> Vec<(String, String)>
pub fn adapter_for_ext(ext: &str) -> Option<&'static dyn LanguageAdapter>
pub fn adapter_for_path(path: &Path) -> Option<&'static dyn LanguageAdapter>
pub struct BslAdapter
pub struct MdoObject
pub fn id(&self) -> String
pub fn parse_mdo_xml(xml: &str) -> Result<MdoObject>
pub fn semantic_diff(old: Option<&MdoObject>, new: &MdoObject) -> Vec<String>
pub fn render_mdo_l2_card(obj: &MdoObject) -> String
pub(crate) fn discover_mdo_subsystems(root: &Path, files: &[String], ) -> (Vec<DiscoveredModule>, BTreeSet<String>)
pub fn find_object_card(root: &Path, scope: &ScopeConfig, id: &str) -> Result<Option<String>>
pub struct PythonAdapter
pub struct RustAdapter
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
