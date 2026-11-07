---
directory: crates/ci-core/src/git.rs
source:
  kind: git
  blob_sha: dd7834620cdccd171a610b3da48a8380dfc95373
  commit_sha: 05b17436dd0087469fb8bef6c61f16e554831faa
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

# Подсистема: Ci Core Git (L1)

Модуль-файл `crates/ci-core/src/git.rs`

> **Идентификатор:** `crates_ci-core_git`
> **Теги:** Rust (Cargo), crates_ci-core_git

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/src/git.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub struct GitStatusEntry
pub struct RepoGitSnapshot
pub struct StalenessReport
pub trait GitBackend
pub fn resolve_git_dir(repo_root: &Path) -> Option<PathBuf>
pub fn resolve_common_git_dir(git_dir: &Path) -> PathBuf
pub fn read_head_commit_in_process(repo_root: &Path) -> Option<String>
pub fn read_remote_url_in_process(repo_root: &Path) -> Option<String>
pub struct SystemGitBackend
pub fn parse_porcelain_z(bytes: &[u8]) -> Vec<GitStatusEntry>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
