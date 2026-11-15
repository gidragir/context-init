---
directory: crates/ci-core/src/analysis
source:
  kind: git
  blob_sha: d041e0059eb1c21b8c45c04f85f9dfd6f79963a0
  commit_sha: 2d4b175053bd98a7be76337518966012c06201a1
generated_by:
  component: context-init
  role: update
  model: none
freshness:
  total_entries: 2
  sampled_entries: 2
  unsampled_entries: 0
  pending_child_changes: 0
---

# Подсистема: Ci Core Analysis Detector (L1)

Модуль и компоненты директории `crates/ci-core/src/analysis/detector` (2 файлов)

> **Идентификатор:** `crates_ci-core_analysis_detector`
> **Теги:** Rust (Cargo), crates_ci-core_analysis_detector

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/src/analysis/detector.rs` | Исходный код |
| `crates/ci-core/src/analysis/detector/mise.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub(crate) const IGNORED_DIRS: &[&str] = &[".git", ".agents", ".context-init", ".gemini", "node_modules", "target", "dist", "build", "gradle", ".gradle", ".idea", ".vscode", ".cargo", "vendor", "__pycache__", ".venv", "venv", ".next", ".nuxt", ".turbo", ".cache", "coverage", ".mypy_cache", ".pytest_cache", ".ruff_cache", ".vault", "out", ".svelte-kit", ".astro", ".parcel-cache", ".agent", ".claude", ".cursor", ".windsurf", ".github", ".changeset", ".deploy", ".impeccable", ".ipynb_checkpoints", ".Trash-1000", ".cargo-cache", ".pnpm-store", ".sccache", ".uv-cache", ]
pub const SOURCE_CODE_EXTENSIONS: &[&str] = &["rs", "py", "ts", "tsx", "js", "jsx", "mjs", "cjs", "mts", "cts", "astro", "vue", "svelte", "go", "bsl", "os", "mdo", "java", "kt", "kts", "cs", "c", "cpp", "h", "hpp", "sh", "bash", "zsh", "fish", "ps1", "psm1", "lua", "nu", "sql", "prisma", "graphql", "proto", "css", "scss", ]
pub const CONFIG_AND_DOC_EXTENSIONS: &[&str] = &["toml", "yaml", "yml", "json", "xml", "md", "mdc", "kdl", "just", ]
pub const CODE_EXTENSIONS: &[&str] = &["rs", "py", "ts", "tsx", "js", "jsx", "mjs", "cjs", "mts", "cts", "astro", "vue", "svelte", "go", "bsl", "os", "mdo", "java", "kt", "kts", "cs", "c", "cpp", "h", "hpp", "sh", "bash", "zsh", "fish", "ps1", "psm1", "lua", "nu", "sql", "prisma", "graphql", "proto", "css", "scss", "toml", "yaml", "yml", "json", "xml", "md", "mdc", "kdl", "just", ]
pub const MIN_SINGLE_FILE_SUBSYSTEM_TOKENS: usize = 1_000
pub fn estimate_tokens_fast(text: &str) -> usize
pub struct AnalyzeOptions
pub fn analyze_project(root: &Path, deep: bool, with_rtk: bool) -> Result<ProjectAnalysis>
pub fn analyze_project_with(root: &Path, opts: &AnalyzeOptions) -> Result<ProjectAnalysis>
pub fn analyze_project_with_cache(root: &Path, opts: &AnalyzeOptions, metrics_cache_dir: Option<&Path>, ) -> Result<ProjectAnalysis>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
