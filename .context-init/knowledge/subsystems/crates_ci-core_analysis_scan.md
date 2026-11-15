---
directory: crates/ci-core/src/analysis/scan.rs
source:
  kind: git
  blob_sha: d01ad46e535aad6db3b79927aa5e068941fc97fe
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
pub(crate) const BINARY_SNIFF_BYTES: usize = 8192
pub fn is_likely_binary(path: &Path) -> bool
pub(crate) fn has_binary_extension(path: &Path) -> bool
pub(crate) fn header_is_binary(header: &[u8]) -> bool
pub struct ScanOptions
pub struct ScanOutlier
pub struct ScanStat
pub struct ScanReport
pub fn scan_project(root: &Path, scope: &ScopeConfig) -> Result<ScanReport>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
