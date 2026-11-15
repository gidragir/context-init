---
directory: crates/ci-core/src/analysis/cache.rs
source:
  kind: git
  blob_sha: cb762b443f363754ae20300d06ec1d0e47b235a4
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

# Подсистема: Ci Core Analysis Cache (L1)

Модуль-файл `crates/ci-core/src/analysis/cache.rs`

> **Идентификатор:** `crates_ci-core_analysis_cache`
> **Теги:** Rust (Cargo), crates_ci-core_analysis_cache

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/src/analysis/cache.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub const METRICS_CACHE_FILE: &str = "metrics_v1.json"
pub const RACY_WINDOW_NANOS: u64 = 2_000_000_000
pub struct FileMetrics
pub struct CachedFileEntry
pub fn sha256_hex(bytes: &[u8]) -> String
pub fn project_hash(root: &Path) -> String
pub struct DiskMetricsCache
pub struct AnalyzedFileCache
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
