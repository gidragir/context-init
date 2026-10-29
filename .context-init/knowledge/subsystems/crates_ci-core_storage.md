---
directory: crates/ci-core/src/storage.rs
source:
  kind: git
  blob_sha: 35508ead97029b6a8476089532c2462f21213db2
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

# Подсистема: Ci Core Storage (L1)

Модуль-файл `crates/ci-core/src/storage.rs`

> **Идентификатор:** `crates_ci-core_storage`
> **Теги:** Rust (Cargo), crates_ci-core_storage

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
