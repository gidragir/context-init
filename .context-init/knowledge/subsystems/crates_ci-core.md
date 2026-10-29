---
directory: crates/ci-core
source:
  kind: git
  blob_sha: 7547494990750dd175e67f0ee051420f89a949a9
  commit_sha: 14efb361ef43dd6f73ad66906402e36beb29aa4a
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
