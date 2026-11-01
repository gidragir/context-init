---
directory: crates/ci-core
source:
  kind: git
  blob_sha: 94a7ac2f66cbbf2a7f9aecdaa6816651d5e297a5
  commit_sha: cd776cdaf43d357c59b46fea99176db18588f7b5
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
