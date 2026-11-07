---
directory: crates/ci-llm
source:
  kind: git
  blob_sha: 9ca137d270aadc75bc3f88a4545cf017b0b45b15
  commit_sha: 05b17436dd0087469fb8bef6c61f16e554831faa
generated_by:
  component: context-init
  role: init
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
