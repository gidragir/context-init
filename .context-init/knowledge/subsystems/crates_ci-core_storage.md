# Подсистема: Ci Core Storage (L1)

> **Идентификатор:** `crates_ci-core_storage`
> **Теги:** Rust (Cargo), crates_ci-core_storage

## 🎯 Назначение и роль
Модуль-файл `crates/ci-core/src/storage.rs`

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/src/storage.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub const CURRENT_L0_SCHEMA_VERSION: u32 = 3
pub fn normalize_path(path: impl AsRef<Path>) -> String
pub fn normalize_path_str(raw: &str) -> String
pub fn normalize_line_endings(text: &str) -> String
pub fn normalize_git_remote(remote_url: &str) -> String
pub fn compute_project_id(project_path: &Path, remote_url: Option<&str>) -> String
pub fn hash_project_id(project_id: &str) -> String
pub enum ProjectType
pub struct SubsystemFile
pub struct SubsystemInfo
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
