---
directory: crates/ci-mcp/src/handlers.rs
source:
  kind: git
  blob_sha: e5b7656c8e371b77a8289a12faf94fc15a04324a
  commit_sha: 9660354a570d6203128acd3df79cd62f23660f76
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
