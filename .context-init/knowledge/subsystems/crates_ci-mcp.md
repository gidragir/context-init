---
directory: crates/ci-mcp
source:
  kind: git
  blob_sha: a8f7a78b3f32ea52e4782594e064f6b18bdd08d4
  commit_sha: d8acb6e026f3c5cf497ef12860f10754102db968
generated_by:
  component: context-init
  role: update
  model: none
freshness:
  total_entries: 2
  sampled_entries: 2
  unsampled_entries: 0
  pending_child_changes: 0
---

# Подсистема: ci-mcp (L1)

MCP-сервер context-init: параметры и результаты инструментов (McpCheckParams, McpGetSubsystemsParams и др.) в tools.rs.

> **Идентификатор:** `crates_ci-mcp`
> **Теги:** crate-root, mcp, tools

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-mcp/Cargo.toml` | ⭐ Точка входа |
| `crates/ci-mcp/src/lib.rs` | ⭐ Точка входа |

## 🔑 Ключевой контракт и экспорты
```text
pub const VERSION: &str = env !("CARGO_PKG_VERSION")
pub fn crate_info() -> String
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-mcp
```
