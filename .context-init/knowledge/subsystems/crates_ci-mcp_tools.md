---
directory: crates/ci-mcp/src/tools.rs
source:
  kind: git
  blob_sha: 027ef2ca662324657b74b49c694668245ff9363b
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

# Подсистема: Ci Mcp Tools (L1)

Модуль-файл `crates/ci-mcp/src/tools.rs`

> **Идентификатор:** `crates_ci-mcp_tools`
> **Теги:** Rust (Cargo), crates_ci-mcp_tools

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-mcp/src/tools.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub struct McpCheckParams
pub struct McpCheckResult
pub struct L0SubsystemSummary
pub struct McpGetSubsystemsParams
pub struct McpGetSubsystemParams
pub struct McpPackParams
pub struct McpSubmitParams
pub struct SubmitResult
pub struct McpFeedbackParams
pub struct FeedbackResult
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-mcp
```
