---
directory: crates/ci-core/src/git.rs
source:
  kind: git
  blob_sha: bd43625693b636a2394c45e6afae2376578612d2
  commit_sha: ebcdb066e85279ae873a213c32310748678eefe2
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
pub struct StalenessReport
pub trait GitBackend
pub struct SystemGitBackend
pub fn parse_porcelain_z(bytes: &[u8]) -> Vec<GitStatusEntry>
pub fn parse_nul_paths(bytes: &[u8]) -> Vec<String>
pub fn check_card_staleness(backend: &dyn GitBackend, repo_root: &Path, card: &OkfCard, ) -> Result<StalenessReport>
pub fn check_frontmatter_staleness(backend: &dyn GitBackend, repo_root: &Path, frontmatter: &CardFrontmatter, ) -> Result<StalenessReport>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
