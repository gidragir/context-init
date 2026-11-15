---
directory: crates/ci-core/src/git.rs
source:
  kind: git
  blob_sha: 253c3f932390492ab395526dfd81fc1bb656dbe6
  commit_sha: a37e2f209483ac97bb4729dec70449be893293c4
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
pub struct GitTreeEntry
pub type DiffNameCache = BTreeMap<String, Option<Vec<String>>>
pub struct RepoGitSnapshot
pub fn snapshot_paths_tree_sha(backend: &dyn GitBackend, repo_root: &Path, snapshot: &RepoGitSnapshot, paths: &[String], ) -> Result<String>
pub struct StalenessReport
pub trait GitBackend
pub fn resolve_git_dir(repo_root: &Path) -> Option<PathBuf>
pub fn resolve_common_git_dir(git_dir: &Path) -> PathBuf
pub fn read_head_commit_in_process(repo_root: &Path) -> Option<String>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
