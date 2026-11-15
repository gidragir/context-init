---
directory: crates/ci-core/src/analysis/lang/text.rs
source:
  kind: git
  blob_sha: ba958a3f0770e8b8d292e778843df41419c2599e
  commit_sha: 8edfd693b819f9b4b54f664fed4f61310c0300c2
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

# Подсистема: Ci Core Analysis Lang Text (L1)

Модуль-файл `crates/ci-core/src/analysis/lang/text.rs`

> **Идентификатор:** `crates_ci-core_analysis_lang_text`
> **Теги:** Rust (Cargo), crates_ci-core_analysis_lang_text

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/src/analysis/lang/text.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub(crate) enum Backtick
pub(crate) struct Flavor
pub(crate) fn mask_code(source: &str, f: Flavor) -> Vec<String>
pub(crate) struct LineRules
pub(crate) fn go_private(t: &str) -> bool
pub(crate) fn apply_options(text: &str, opts: &CompressOptions, rules: &LineRules) -> String
pub(crate) type DetectFn<'a> = &'a dyn Fn(&str, Option<&'static str>) -> Option<(&'static str, bool)>
pub(crate) struct SpanRules<'a>
pub(crate) fn signature_text(lines: &[&str], start: usize, sig_line: usize, sig_col: usize, ) -> String
pub(crate) fn brace_spans(source: &str, masked: &[String], rules: &SpanRules<'_>, ) -> Vec<SymbolSpan>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
