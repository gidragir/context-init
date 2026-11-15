---
directory: crates/ci-llm
source:
  kind: git
  blob_sha: b5db60afe3867c39e02d7b4acee8735e873c3591
  commit_sha: 7a0150d418cd748d8858807e89a06e8a7225091e
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

# Подсистема: Ci Llm (L1)

Подсистема `crates/ci-llm` с ключевыми точками входа: crates/ci-llm/Cargo.toml, crates/ci-llm/src/lib.rs

> **Идентификатор:** `crates_ci-llm`
> **Теги:** Rust (Cargo), crates_ci-llm

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-llm/Cargo.toml` | ⭐ Точка входа |
| `crates/ci-llm/src/lib.rs` | ⭐ Точка входа |
| `crates/ci-llm/src/provider.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub const VERSION: &str = env !("CARGO_PKG_VERSION")
pub fn crate_info() -> String
pub type RoleName = RoleKind
pub struct AgentWorkPackage
pub struct ValidatedAgentOutput
pub struct LlmRequest
pub struct LlmResponse
pub struct ProviderCapabilities
pub trait LlmProvider
pub struct LlmExecutionError
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-llm
```
