---
directory: crates/ci-core
source:
  kind: git
  blob_sha: 6d9e8de0b44d023be0d9590cb9f4574bcbd1ccf2
  commit_sha: 9660354a570d6203128acd3df79cd62f23660f76
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

# Подсистема: ci-core (L1)

Ядро без LLM: версия крейта, трейт Retriever и SearchHit для поиска по базе знаний; реэкспорт модулей analysis, config, git, storage, openviking, telemetry.

> **Идентификатор:** `crates_ci-core`
> **Теги:** core, crate-root, search

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
