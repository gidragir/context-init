---
directory: crates/ci-core
source:
  kind: git
  blob_sha: a052247c0bb7b21b05ac6dac6e0c16332564ae1a
  commit_sha: d8acb6e026f3c5cf497ef12860f10754102db968
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
