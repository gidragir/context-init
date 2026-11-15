---
directory: crates/ci-mcp/src/handlers.rs
source:
  kind: git
  blob_sha: cfe9dccedbc94f9a4305ec6ef302fa1b6306c255
  commit_sha: a37e2f209483ac97bb4729dec70449be893293c4
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
pub struct SubmitReply
pub struct ContextHandler
pub struct PackHandlerParams<'a>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-mcp
```
