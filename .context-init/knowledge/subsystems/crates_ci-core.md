---
directory: crates/ci-core
source:
  kind: git
  blob_sha: 8c4b238e02b3118d61c065be2556712c6118bace
  commit_sha: 6ef401530ecaf2a5ec6c1476e8b34c2cdb8a29d7
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
