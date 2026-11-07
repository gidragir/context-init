---
directory: crates/ci-core/src/analysis/lang
source:
  kind: git
  blob_sha: 7e616e3cf5dd016e1c3a839d487d75f22352b28d
  commit_sha: d8acb6e026f3c5cf497ef12860f10754102db968
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

# Подсистема: Языковые адаптеры (L1)

Трейт LanguageAdapter (language_id, extensions, extract_symbols) и адаптеры Rust, Python, BSL и MDO (1С); ELIDED_BODY — маркер вырезанного тела при сжатии.

> **Идентификатор:** `crates_ci-core_analysis_lang`
> **Теги:** 1c, ast, languages, python, rust

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/src/analysis/lang/brace.rs` | Исходный код |
| `crates/ci-core/src/analysis/lang/bsl.rs` | Исходный код |
| `crates/ci-core/src/analysis/lang/kotlin.rs` | Исходный код |
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
pub struct BraceLangAdapter
pub struct BslAdapter
pub struct KotlinAdapter
pub struct MdoObject
pub fn parse_mdo_xml(xml: &str) -> Result<MdoObject>
pub fn semantic_diff(old: Option<&MdoObject>, new: &MdoObject) -> Vec<String>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
