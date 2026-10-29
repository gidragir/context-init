---
directory: crates/ci-core/src/analysis/detector.rs
source:
  kind: git
  blob_sha: 9b802423c0c493e9cd8b76796f195f34f70e74ee
  commit_sha: 36198169b6c3e1dac832cf4204e7b96c6805df94
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

# Подсистема: Ci Core Analysis Detector (L1)

Модуль и компоненты директории `crates/ci-core/src/analysis/detector` (1 файлов)

> **Идентификатор:** `crates_ci-core_analysis_detector`
> **Теги:** Rust (Cargo), crates_ci-core_analysis_detector

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
pub fn count_tokens(text: &str) -> usize
pub fn is_ignored_path(path: &Path, root: &Path) -> bool
pub fn extract_rust_symbols(content: &str, deep: bool) -> Vec<String>
pub fn extract_multiline_symbols(content: &str, ext: &str, deep: bool) -> Vec<String>
pub fn clean_symbol_line(line: &str) -> String
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
