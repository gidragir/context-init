---
directory: crates/ci-mcp
source:
  kind: git
  blob_sha: e5ce15d56c89d006807d16433d46a2d27574429e
  commit_sha: fff7dd106863757218c552cb8ef5b18e46c70899
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
