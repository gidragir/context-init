---
directory: crates/ci-core/src/storage.rs
source:
  kind: git
  blob_sha: b7a79a403fdae35d5b3b255e692e9d7bac460ff9
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

# Подсистема: Хранилище (L1)

L0-индекс (схема v3), нормализация путей, переводов строк и git remote; идентификация проекта для центрального хранилища в XDG data dir.

> **Идентификатор:** `crates_ci-core_storage`
> **Теги:** l0-index, paths, storage

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/src/storage.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub const CURRENT_L0_SCHEMA_VERSION: u32 = 4
pub fn normalize_path(path: impl AsRef<Path>) -> String
pub fn normalize_path_str(raw: &str) -> String
pub fn normalize_line_endings(text: &str) -> String
pub fn normalize_git_remote(remote_url: &str) -> String
pub fn compute_project_id(project_path: &Path, remote_url: Option<&str>) -> String
pub fn hash_project_id(project_id: &str) -> String
pub enum ProjectType
pub struct SubsystemFile
pub enum SummarySource
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
