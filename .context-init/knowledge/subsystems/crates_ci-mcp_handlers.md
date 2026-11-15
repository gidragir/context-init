---
directory: crates/ci-mcp/src/handlers.rs
source:
  kind: git
  blob_sha: 38b04455c6c931eef1cbfb3db52ff9fa7aa16cd7
  commit_sha: 3dd948d5e6098dc9ab728abc589a8352d670968f
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
