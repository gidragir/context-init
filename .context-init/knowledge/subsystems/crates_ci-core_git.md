---
directory: crates/ci-core/src/git.rs
source:
  kind: git
  blob_sha: 29580eea882d7de74d703cc9f25d12a187e2d814
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

# Подсистема: Git-бэкенд (L1)

Трейт GitBackend (head_commit, статус, diff), GitStatusEntry и StalenessReport для определения устаревания карточек относительно коммитов.

> **Идентификатор:** `crates_ci-core_git`
> **Теги:** git, staleness

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
