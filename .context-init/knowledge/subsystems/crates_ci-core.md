---
directory: crates/ci-core
source:
  kind: git
  blob_sha: 1d308b6c64f6d70083112017a0fe1f2165f1b4ed
  commit_sha: 36198169b6c3e1dac832cf4204e7b96c6805df94
generated_by:
  component: context-init
  role: init
  model: none
freshness:
  total_entries: 3
  sampled_entries: 3
  unsampled_entries: 0
  pending_child_changes: 0
---

# Подсистема: Ci Core (L1)

Подсистема `crates/ci-core` с ключевыми точками входа: crates/ci-core/Cargo.toml, crates/ci-core/src/lib.rs

> **Идентификатор:** `crates_ci-core`
> **Теги:** Rust (Cargo), crates_ci-core

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/Cargo.toml` | ⭐ Точка входа |
| `crates/ci-core/src/git.rs` | Исходный код |
| `crates/ci-core/src/lib.rs` | ⭐ Точка входа |

## 🔑 Ключевой контракт и экспорты
```text
pub const VERSION: &str = env !("CARGO_PKG_VERSION")
pub fn crate_info() -> &'static str
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
