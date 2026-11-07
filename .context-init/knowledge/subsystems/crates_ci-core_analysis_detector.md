---
directory: crates/ci-core/src/analysis
source:
  kind: git
  blob_sha: 4bf0bc416b8cb4383227bb9172024a529f4b8b3c
  commit_sha: d8acb6e026f3c5cf497ef12860f10754102db968
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

# Подсистема: Детектор проекта (L1)

analyze_project / analyze_project_with: определение типа проекта, команд, подсистем и сигнатур (deep); список IGNORED_DIRS и префикс rtk для генерируемых команд.

> **Идентификатор:** `crates_ci-core_analysis_detector`
> **Теги:** detector, project-type, rtk

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
pub fn prefix_with_rtk(cmd: &str) -> String
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
