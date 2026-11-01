---
directory: crates/ci-core/src/config.rs
source:
  kind: git
  blob_sha: 0b26fdd4a332dd56535180703c50092c607f466e
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

# Подсистема: Конфигурация (L1)

Каскад конфигурации (глобальный, проектный, --config, env, CLI): язык (ru по умолчанию), бюджет ответа 4000 токенов, порог разбиения 15000, модели [models], роли [roles] (none = детерминированно), интеграции и телеметрия.

> **Идентификатор:** `crates_ci-core_config`
> **Теги:** cascade, config, models, roles

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
