# Подсистема: Ci Core (L1)

> **Идентификатор:** `crates_ci-core`
> **Теги:** Rust (Cargo), crates_ci-core

## 🎯 Назначение и роль
Подсистема `crates/ci-core` с ключевыми точками входа: crates/ci-core/Cargo.toml, crates/ci-core/src/lib.rs

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
