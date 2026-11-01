---
directory: crates/ci-core/src/config.rs
source:
  kind: git
  blob_sha: 25424609f81ba0c23917dd4ffdbf8438432e0f88
  commit_sha: fff7dd106863757218c552cb8ef5b18e46c70899
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
pub fn resolve(repo_root: Option<&Path>) -> Self
pub fn resolve_with_env<F>(repo_root: Option<&Path>, home_dir: Option<&Path>, get_env: F, ) -> Self where F: Fn(&str) -> Option<String>,
pub fn project_storage_dir(&self, project_id: &str) -> PathBuf
pub fn project_knowledge_dir(&self, project_id: &str) -> PathBuf
pub fn effective_global_config_path(&self) -> &Path
pub fn format_paths(&self, project_id: Option<&str>) -> String
pub enum RtkMode
pub fn as_str(&self) -> &'static str
pub fn is_active(&self) -> bool
pub fn is_active_with_path(&self, path_var: Option<&OsStr>) -> bool
pub enum StorageMode
pub fn as_str(self) -> &'static str
pub fn is_binary_available(bin_name: &str) -> bool
pub fn is_binary_available_in_path(bin_name: &str, path_var: Option<&OsStr>) -> bool
pub fn is_binary_available_in_dirs<I, P>(bin_name: &str, dirs: I) -> bool where I: IntoIterator<Item = P>, P: AsRef<Path>,
pub fn is_rtk_available() -> bool
pub fn resolve_rtk_active(mode: RtkMode) -> bool
pub fn resolve_rtk_active_with_path(mode: RtkMode, path_var: Option<&OsStr>) -> bool
pub struct IntegrationsConfig
pub fn resolve_rtk_active(&self) -> bool
pub fn resolve_rtk_active_with_path(&self, path_var: Option<&OsStr>) -> bool
pub enum ModelKind
pub struct ModelConfig
pub enum RoleKind
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
