---
directory: crates/ci-llm
source:
  kind: git
  blob_sha: 68a5991acf28a3d0e2463c1c7d17407e35ef5358
  commit_sha: a37e2f209483ac97bb4729dec70449be893293c4
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
