---
directory: crates/ci-mcp
source:
  kind: git
  blob_sha: 0c5209699f2142239ef2e7f4626d94b14b940f4c
  commit_sha: 9660354a570d6203128acd3df79cd62f23660f76
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
