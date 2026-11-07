---
directory: crates/ci-core/src/analysis
source:
  kind: git
  blob_sha: 4bf0bc416b8cb4383227bb9172024a529f4b8b3c
  commit_sha: 05b17436dd0087469fb8bef6c61f16e554831faa
generated_by:
  component: context-init
  role: init
  model: none
freshness:
  total_entries: 6
  sampled_entries: 6
  unsampled_entries: 0
  pending_child_changes: 0
---

# Подсистема: Ci Core Analysis (L1)

Подсистема `crates/ci-core/src/analysis` с ключевыми точками входа: crates/ci-core/src/analysis/mod.rs

> **Идентификатор:** `crates_ci-core_analysis`
> **Теги:** Rust (Cargo), crates_ci-core_analysis

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/src/analysis/file_kind.rs` | Исходный код |
| `crates/ci-core/src/analysis/mod.rs` | ⭐ Точка входа |
| `crates/ci-core/src/analysis/scope.rs` | Исходный код |
| `crates/ci-core/src/analysis/subsystems.rs` | Исходный код |
| `crates/ci-core/src/analysis/types.rs` | Исходный код |
| `crates/ci-core/src/analysis/workspace.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub const REPO_KNOWLEDGE_DIR: &str = ".context-init/knowledge"
pub fn resolve_knowledge_dir(root: &Path, mode: StorageMode, central_kb: &Path) -> PathBuf
pub fn canonical_file_ext(rel_path: &str) -> Option<String>
pub const ALWAYS_IGNORED_RUNTIME_DIRS: &[&str] = &[".git", ".context-init", "node_modules", "target", ".venv", "venv", ".cache", ".worktrees", ".claude/worktrees", ]
pub const IGNORED_LOCK_AND_ARTIFACT_FILES: &[&str] = &["Cargo.lock", "pnpm-lock.yaml", "package-lock.json", "yarn.lock", "bun.lock", "bun.lockb", "uv.lock", "poetry.lock", "Pipfile.lock", "composer.lock", "Gemfile.lock", "flake.lock", "skills-lock.json", "lazy-lock.json", ]
pub const IGNORED_FILE_SUFFIXES: &[&str] = &[".tsbuildinfo", ".min.js", ".min.css", ".bundle.js", ".js.map", ".css.map", ".d.ts.map", ]
pub fn is_ignored_filename(name: &str) -> bool
pub(crate) fn include_allows_dir_traversal(rel_dir: &str, patterns: &[String]) -> bool
pub fn validate_scope(scope: &ScopeConfig) -> Result<()>
pub struct ScopeFilter
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
