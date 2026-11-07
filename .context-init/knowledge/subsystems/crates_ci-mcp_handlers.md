---
directory: crates/ci-mcp/src/handlers.rs
source:
  kind: git
  blob_sha: 5d914f6dd2462e99dcb0a795cea694707eba32dd
  commit_sha: d8acb6e026f3c5cf497ef12860f10754102db968
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

# Подсистема: Обработчики MCP (L1)

ResolvedProject (загрузка конфига и базы), обработчики get_subsystems/get_subsystem/pack/search/task/check/submit; ограничение pack all (5 страниц), сохранение агентских submission с проверкой id.

> **Идентификатор:** `crates_ci-mcp_handlers`
> **Теги:** handlers, mcp, submit

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
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-mcp
```
