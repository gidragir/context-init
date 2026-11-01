---
directory: crates/ci-core/src/analysis/detector.rs
source:
  kind: git
  blob_sha: cf3c8a1d022ff391675305a4f19e0d76dc6c284f
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

# Подсистема: Детектор проекта (L1)

analyze_project / analyze_project_with: определение типа проекта, команд, подсистем и сигнатур (deep); список IGNORED_DIRS и префикс rtk для генерируемых команд.

> **Идентификатор:** `crates_ci-core_analysis_detector`
> **Теги:** detector, project-type, rtk

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/src/analysis/detector.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub(crate) const IGNORED_DIRS: &[&str] = &[".git", ".agents", ".context-init", ".gemini", "node_modules", "target", "dist", "build", ".idea", ".vscode", ".cargo", "vendor", "__pycache__", ".venv", "venv", ".next", ".nuxt", ".turbo", ".cache", "coverage", ".mypy_cache", ".pytest_cache", ".ruff_cache", ".vault", "out", ".svelte-kit", ".astro", ".parcel-cache", ]
pub struct AnalyzeOptions
pub fn analyze_project(root: &Path, deep: bool, with_rtk: bool) -> Result<ProjectAnalysis>
pub fn analyze_project_with(root: &Path, opts: &AnalyzeOptions) -> Result<ProjectAnalysis>
pub fn prefix_with_rtk(cmd: &str) -> String
pub fn is_ignored_path(path: &Path, root: &Path) -> bool
pub fn is_test_path(rel: &str) -> bool
pub fn extract_rust_symbols(content: &str, deep: bool) -> Vec<String>
pub fn extract_multiline_symbols(content: &str, ext: &str, deep: bool) -> Vec<String>
pub fn clean_symbol_line(line: &str) -> String
pub fn sanitize_symbol(raw: &str) -> Option<String>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
