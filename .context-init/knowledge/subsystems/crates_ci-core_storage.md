---
directory: crates/ci-core/src/storage.rs
source:
  kind: git
  blob_sha: 35508ead97029b6a8476089532c2462f21213db2
  commit_sha: 4f153c6ee310b681808626eb7348ac3ca484e21d
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
pub fn display_name(&self) -> &'static str
pub struct SubsystemFile
pub struct SubsystemInfo
pub const SUBSYSTEM_ID_PATTERN: &str = "^[a-z0-9][a-z0-9_-]{0,63}$"
pub fn validate_subsystem_id(id: &str) -> Result<()>
pub fn subsystem_card_path(knowledge_dir: &Path, id: &str) -> Result<PathBuf>
pub fn safe_join(base: &Path, rel: &str) -> Option<PathBuf>
pub fn now_rfc3339() -> String
pub struct ProjectCommands
pub struct L0Index
pub fn from_json_str(json: &str) -> Result<Self>
pub fn load_from_file(path: &Path) -> Result<Self>
pub fn to_json_pretty(&self) -> Result<String>
pub fn save_to_file(&self, path: &Path) -> Result<()>
pub struct DriftReport
pub struct CopyKbOptions
pub struct CopyKbReport
pub fn copy_knowledge(src: &Path, dst: &Path, opts: &CopyKbOptions) -> Result<CopyKbReport>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
