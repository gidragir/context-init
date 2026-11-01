---
directory: crates/ci-core/src/analysis/detector.rs
source:
  kind: git
  blob_sha: a0a985836fef1704c8d780d568fcc27af1004ef4
  commit_sha: 9660354a570d6203128acd3df79cd62f23660f76
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
pub(crate) const IGNORED_DIRS: &[&str] = &[".git", ".agents", ".context-init", ".gemini", "node_modules", "target", "dist", "build", ".idea", ".vscode", ".cargo", "vendor", "__pycache__", ".venv", "venv", ".next", ".nuxt", ".turbo", ".cache", "coverage", ".mypy_cache", ".pytest_cache", ".ruff_cache", ".vault", "out", ".svelte-kit", ".astro", ".parcel-cache", ".agent", ".claude", ".cursor", ".windsurf", ".github", ".changeset", ".deploy", ".impeccable", ".ipynb_checkpoints", ".Trash-1000", ".cargo-cache", ".pnpm-store", ".sccache", ".uv-cache", ]
pub const SOURCE_CODE_EXTENSIONS: &[&str] = &["rs", "py", "ts", "tsx", "js", "jsx", "mjs", "cjs", "mts", "cts", "astro", "vue", "svelte", "go", "bsl", "os", "mdo", "java", "kt", "kts", "cs", "c", "cpp", "h", "hpp", "sh", "bash", "zsh", "fish", "ps1", "psm1", "lua", "nu", "sql", "prisma", "graphql", "proto", "css", "scss", ]
pub const CONFIG_AND_DOC_EXTENSIONS: &[&str] = &["toml", "yaml", "yml", "json", "xml", "md", "mdc", "kdl", "just", ]
pub const CODE_EXTENSIONS: &[&str] = &["rs", "py", "ts", "tsx", "js", "jsx", "mjs", "cjs", "mts", "cts", "astro", "vue", "svelte", "go", "bsl", "os", "mdo", "java", "kt", "kts", "cs", "c", "cpp", "h", "hpp", "sh", "bash", "zsh", "fish", "ps1", "psm1", "lua", "nu", "sql", "prisma", "graphql", "proto", "css", "scss", "toml", "yaml", "yml", "json", "xml", "md", "mdc", "kdl", "just", ]
pub fn estimate_tokens_fast(text: &str) -> usize
pub struct AnalyzeOptions
pub fn analyze_project(root: &Path, deep: bool, with_rtk: bool) -> Result<ProjectAnalysis>
pub fn analyze_project_with(root: &Path, opts: &AnalyzeOptions) -> Result<ProjectAnalysis>
pub fn prefix_with_rtk(cmd: &str) -> String
pub fn is_ignored_path(path: &Path, root: &Path) -> bool
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
