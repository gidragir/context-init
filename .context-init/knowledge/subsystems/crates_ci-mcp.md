---
directory: crates/ci-mcp
source:
  kind: git
  blob_sha: 91c3a42511050390d85e8d416301a8b0fce6694d
  commit_sha: 9048360bdc3d59b7d897b5cfe7eea7df1cd818a8
generated_by:
  component: context-init
  role: update
  model: none
freshness:
  total_entries: 3
  sampled_entries: 3
  unsampled_entries: 0
  pending_child_changes: 0
---

# Подсистема: Ci Mcp (L1)

Подсистема `crates/ci-mcp` с ключевыми точками входа: crates/ci-mcp/Cargo.toml, crates/ci-mcp/src/lib.rs

> **Идентификатор:** `crates_ci-mcp`
> **Теги:** Rust (Cargo), crates_ci-mcp

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-mcp/Cargo.toml` | ⭐ Точка входа |
| `crates/ci-mcp/src/lib.rs` | ⭐ Точка входа |
| `crates/ci-mcp/src/tools.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub const VERSION: &str = env !("CARGO_PKG_VERSION")
pub fn crate_info() -> String
pub struct McpCheckParams
pub struct McpCheckResult
pub struct McpGetSubsystemsParams
pub struct McpGetSubsystemParams
pub struct McpPackParams
pub struct McpSubmitParams
pub struct SubmitResult
pub struct McpFeedbackParams
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-mcp
```
