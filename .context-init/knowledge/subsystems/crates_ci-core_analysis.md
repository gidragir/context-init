---
directory: crates/ci-core/src/analysis
source:
  kind: git
  blob_sha: c8e674cb9e3bbac934794416348dc2b6013a9847
  commit_sha: 9660354a570d6203128acd3df79cd62f23660f76
generated_by:
  component: context-init
  role: update
  model: none
freshness:
  total_entries: 6
  sampled_entries: 6
  unsampled_entries: 0
  pending_child_changes: 0
---

# Подсистема: Анализ проекта (L1)

Сканирование дерева (scan_project, ScanReport), конфиг области (scope), выделение подсистем и задачный дайджест; resolve_knowledge_dir выбирает repo (.context-init/knowledge) или центральное хранилище.

> **Идентификатор:** `crates_ci-core_analysis`
> **Теги:** analysis, scan, storage-mode, subsystems

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/src/analysis/mod.rs` | ⭐ Точка входа |
| `crates/ci-core/src/analysis/scan.rs` | Исходный код |
| `crates/ci-core/src/analysis/scope.rs` | Исходный код |
| `crates/ci-core/src/analysis/subsystems.rs` | Исходный код |
| `crates/ci-core/src/analysis/types.rs` | Исходный код |
| `crates/ci-core/src/analysis/workspace.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub const REPO_KNOWLEDGE_DIR: &str = ".context-init/knowledge"
pub fn resolve_knowledge_dir(root: &Path, mode: StorageMode, central_kb: &Path) -> PathBuf
pub const KNOWN_BINARY_EXTENSIONS: &[&str] = &["exe", "dll", "so", "dylib", "msi", "cab", "zip", "tar", "gz", "7z", "rar", "png", "jpg", "jpeg", "gif", "webp", "ico", "icns", "pdf", "woff", "woff2", "ttf", "otf", "eot", "mp3", "mp4", "wav", "ogg", "wasm", "pyc", "class", "jar", "db", "sqlite", "sqlite3", "cf", "epf", "erf", "bin", "pack", "idx", ]
pub fn is_likely_binary(path: &Path) -> bool
pub struct ScanStat
pub struct ScanReport
pub fn scan_project(root: &Path, scope: &ScopeConfig) -> Result<ScanReport>
pub fn render_scan_markdown(project_id: &str, root: &Path, report: &ScanReport) -> String
pub const IGNORED_LOCK_AND_ARTIFACT_FILES: &[&str] = &["Cargo.lock", "pnpm-lock.yaml", "package-lock.json", "yarn.lock", "bun.lock", "bun.lockb", "uv.lock", "poetry.lock", "Pipfile.lock", "composer.lock", "Gemfile.lock", "flake.lock", "skills-lock.json", "lazy-lock.json", ]
pub const IGNORED_FILE_SUFFIXES: &[&str] = &[".tsbuildinfo", ".min.js", ".min.css", ".bundle.js", ".js.map", ".css.map", ".d.ts.map", ]
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
