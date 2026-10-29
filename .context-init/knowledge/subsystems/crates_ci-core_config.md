---
directory: crates/ci-core/src/config.rs
source:
  kind: git
  blob_sha: 25424609f81ba0c23917dd4ffdbf8438432e0f88
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

# Подсистема: Ci Core Config (L1)

Модуль-файл `crates/ci-core/src/config.rs`

> **Идентификатор:** `crates_ci-core_config`
> **Теги:** Rust (Cargo), crates_ci-core_config

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/src/config.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub const DEFAULT_LANGUAGE: &str = "ru"
pub const DEFAULT_MAX_RESPONSE_TOKENS: usize = 4000
pub const DEFAULT_SPLIT_THRESHOLD_TOKENS: usize = 15_000
pub const MIN_RESPONSE_TOKENS: usize = 64
pub const ROLE_NONE: &str = "none"
pub struct StoragePaths
pub enum RtkMode
pub enum StorageMode
pub fn is_binary_available(bin_name: &str) -> bool
pub fn is_binary_available_in_path(bin_name: &str, path_var: Option<&OsStr>) -> bool
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
