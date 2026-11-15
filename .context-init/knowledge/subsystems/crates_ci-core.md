---
directory: crates/ci-core
source:
  kind: git
  blob_sha: 6893e30949a678861303f3e80665892865bbab11
  commit_sha: 2d4b175053bd98a7be76337518966012c06201a1
generated_by:
  component: context-init
  role: update
  model: none
freshness:
  total_entries: 5
  sampled_entries: 5
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
| `crates/ci-core/build.rs` | Исходный код |
| `crates/ci-core/src/lib.rs` | ⭐ Точка входа |
| `crates/ci-core/src/search.rs` | Исходный код |
| `crates/ci-core/src/tokens.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub const VERSION: &str = env !("CARGO_PKG_VERSION")
pub const BUILD: &str = env !("CONTEXT_INIT_BUILD")
pub fn crate_info() -> &'static str
pub struct SearchHit
pub trait Retriever
pub struct Bm25Retriever
pub fn count_tokens(text: &str) -> usize
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
