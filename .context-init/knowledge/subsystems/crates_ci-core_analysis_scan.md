---
directory: crates/ci-core/src/analysis/scan.rs
source:
  kind: git
  blob_sha: ee9151ff04f939eca86e19c74ce8fb75b8487414
  commit_sha: 05b17436dd0087469fb8bef6c61f16e554831faa
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

# Подсистема: Ci Core Analysis Scan (L1)

Модуль-файл `crates/ci-core/src/analysis/scan.rs`

> **Идентификатор:** `crates_ci-core_analysis_scan`
> **Теги:** Rust (Cargo), crates_ci-core_analysis_scan

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/src/analysis/scan.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub const KNOWN_BINARY_EXTENSIONS: &[&str] = &["exe", "dll", "so", "dylib", "msi", "cab", "zip", "tar", "gz", "7z", "rar", "png", "jpg", "jpeg", "gif", "webp", "ico", "icns", "pdf", "woff", "woff2", "ttf", "otf", "eot", "mp3", "mp4", "wav", "ogg", "wasm", "pyc", "class", "jar", "db", "sqlite", "sqlite3", "cf", "epf", "erf", "bin", "pack", "idx", ]
pub fn is_likely_binary(path: &Path) -> bool
pub struct ScanOptions
pub struct ScanOutlier
pub struct ScanStat
pub struct ScanReport
pub fn scan_project(root: &Path, scope: &ScopeConfig) -> Result<ScanReport>
pub fn scan_project_with(root: &Path, scope: &ScopeConfig, opts: &ScanOptions, ) -> Result<ScanReport>
pub fn render_scan_markdown(project_id: &str, root: &Path, report: &ScanReport) -> String
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
