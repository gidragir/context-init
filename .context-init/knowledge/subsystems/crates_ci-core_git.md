---
directory: crates/ci-core/src/git.rs
source:
  kind: git
  blob_sha: f6ddc39cd6d468d8a2cbf410eed2df16baac9286
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
pub fn status_code(&self) -> String
pub struct StalenessReport
pub trait GitBackend
fn head_commit(&self, repo_root: &Path) -> Result<String>
fn remote_url(&self, repo_root: &Path) -> Result<Option<String>>
fn blob_sha(&self, repo_root: &Path, relative_path: &Path) -> Result<Option<String>>
fn status_porcelain_z(&self, repo_root: &Path) -> Result<Vec<GitStatusEntry>>
fn diff_name_only(&self, repo_root: &Path, from_commit: &str, to_commit: &str, ) -> Result<Vec<String>>
fn diff_stat(&self, repo_root: &Path, from_commit: &str, to_commit: &str, paths: &[String], ) -> Result<String>
fn current_branch(&self, _repo_root: &Path) -> Result<String>
fn merge_base(&self, _repo_root: &Path, _a: &str, _b: &str) -> Result<String>
fn log_oneline(&self, _repo_root: &Path, _from_commit: &str, _to_commit: &str, _limit: usize, ) -> Result<Vec<String>>
fn file_at(&self, _repo_root: &Path, _rev: &str, _path: &str) -> Result<Option<String>>
fn diff_file(&self, _repo_root: &Path, _from_commit: &str, _to_commit: &str, _path: &str, ) -> Result<String>
pub struct SystemGitBackend
pub fn new() -> Self
pub fn with_binary(git_binary: impl Into<String>) -> Self
pub fn parse_porcelain_z(bytes: &[u8]) -> Vec<GitStatusEntry>
pub fn parse_nul_paths(bytes: &[u8]) -> Vec<String>
pub fn check_card_staleness(backend: &dyn GitBackend, repo_root: &Path, card: &OkfCard, ) -> Result<StalenessReport>
pub fn check_frontmatter_staleness(backend: &dyn GitBackend, repo_root: &Path, frontmatter: &CardFrontmatter, ) -> Result<StalenessReport>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
