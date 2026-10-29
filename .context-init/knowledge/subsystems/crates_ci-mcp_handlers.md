---
directory: crates/ci-mcp/src/handlers.rs
source:
  kind: git
  blob_sha: 85b6840db601a513a10687b11f070d36da98f8a7
  commit_sha: 9048360bdc3d59b7d897b5cfe7eea7df1cd818a8
generated_by:
  component: context-init
  role: update
  model: none
freshness:
  total_entries: 1
  sampled_entries: 1
  unsampled_entries: 0
  pending_child_changes: 0
---

# Подсистема: Ci Mcp Handlers (L1)

Модуль-файл `crates/ci-mcp/src/handlers.rs`

> **Идентификатор:** `crates_ci-mcp_handlers`
> **Теги:** Rust (Cargo), crates_ci-mcp_handlers

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-mcp/src/handlers.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub const PACK_ALL_MAX_PAGES: usize = 5
pub struct ResolvedProject
pub const DEFAULT_SEARCH_TOP_K: usize = 10
pub struct HandlerReply
pub struct ContextHandler
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-mcp
```
