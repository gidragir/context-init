---
directory: crates/ci-core
source:
  kind: git
  blob_sha: b6014eeb15748ae8e817ffdea1a6f0bcdd42cf34
  commit_sha: 1727f5aedb34b29bd1ead5d44dd9c3398d8d4dba
generated_by:
  component: context-init
  role: update
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
| `crates/ci-core/src/lib.rs` | ⭐ Точка входа |
| `crates/ci-core/src/search.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub const VERSION: &str = env !("CARGO_PKG_VERSION")
pub fn crate_info() -> &'static str
pub struct SearchHit
pub trait Retriever
pub struct Bm25Retriever
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
