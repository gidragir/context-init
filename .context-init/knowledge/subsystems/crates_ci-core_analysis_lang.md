---
directory: crates/ci-core/src/analysis/lang
source:
  kind: git
  blob_sha: 1dc53458dcd157c34473c3bc6b8f130c226605f9
  commit_sha: ebecc37e77cdf411f5310fea0fda7166b9e27a50
generated_by:
  component: context-init
  role: init
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
| `crates/ci-core/src/analysis/lang/brace.rs` | Исходный код |
| `crates/ci-core/src/analysis/lang/bsl.rs` | Исходный код |
| `crates/ci-core/src/analysis/lang/mod.rs` | ⭐ Точка входа |
| `crates/ci-core/src/analysis/lang/python.rs` | Исходный код |
| `crates/ci-core/src/analysis/lang/rust.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub const ELIDED_BODY: &str = "⋮----"
pub const INLINE_ELIDED: &str = "⋮"
pub struct SymbolSpan
pub trait LanguageAdapter
pub fn symbol_name(span: &SymbolSpan) -> String
pub fn adapter_for_ext(ext: &str) -> Option<&'static dyn LanguageAdapter>
pub fn adapter_for_path(path: &Path) -> Option<&'static dyn LanguageAdapter>
pub struct BraceLangAdapter
pub struct BslAdapter
pub struct PythonAdapter
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
