# Подсистема: Ci Core Analysis Detector (L1)

> **Идентификатор:** `crates_ci-core_analysis_detector`
> **Теги:** Rust (Cargo), crates_ci-core_analysis_detector

## 🎯 Назначение и роль
Модуль и компоненты директории `crates/ci-core/src/analysis/detector` (1 файлов)

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/src/analysis/detector.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub struct AnalyzeOptions
pub fn analyze_project(root: &Path, deep: bool, with_rtk: bool) -> Result<ProjectAnalysis>
pub fn analyze_project_with(root: &Path, opts: &AnalyzeOptions) -> Result<ProjectAnalysis>
pub fn prefix_with_rtk(cmd: &str) -> String
pub fn count_tokens(text: &str) -> usize
pub fn is_ignored_path(path: &Path, root: &Path) -> bool
pub fn extract_rust_symbols(content: &str, deep: bool) -> Vec<String>
pub fn extract_multiline_symbols(content: &str, ext: &str, deep: bool) -> Vec<String>
pub fn clean_symbol_line(line: &str) -> String
pub fn sanitize_symbol(raw: &str) -> Option<String>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
