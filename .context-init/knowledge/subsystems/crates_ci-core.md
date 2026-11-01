---
directory: crates/ci-core
source:
  kind: git
  blob_sha: ad8321fac9563e64f5e1997a3a0f97f3d54989f9
  commit_sha: fff7dd106863757218c552cb8ef5b18e46c70899
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
fn search(&self, query: &str, top_k: usize) -> Result<Vec<SearchHit>>
pub struct Bm25Retriever
pub fn build(index: &L0Index, knowledge_dir: &Path) -> Result<Self>
pub fn count_tokens(text: &str) -> usize
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
