---
directory: crates/ci-mcp/src/tools.rs
source:
  kind: git
  blob_sha: b53d2b979b2ead0a6d8775302ad2384ff38cd74a
  commit_sha: 2ea8f3affd3c7e26633575afd792de4875d13585
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
pub struct McpGetSubsystemsParams
pub struct McpGetSubsystemParams
pub struct McpPackParams
pub struct McpSubmitParams
pub struct SubmitResult
pub struct McpFeedbackParams
pub struct FeedbackResult
pub struct McpInitParams
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-mcp
```
