# Подсистема: Ci Core Config (L1)

> **Идентификатор:** `crates_ci-core_config`
> **Теги:** Rust (Cargo), crates_ci-core_config

## 🎯 Назначение и роль
Модуль-файл `crates/ci-core/src/config.rs`

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
