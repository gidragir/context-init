---
directory: src/cli
source:
  kind: git
  blob_sha: 0ad707ab5158de190029e2c770d41b5f995d830d
  commit_sha: ebcdb066e85279ae873a213c32310748678eefe2
generated_by:
  component: context-init
  role: update
  model: none
freshness:
  total_entries: 13
  sampled_entries: 13
  unsampled_entries: 0
  pending_child_changes: 0
---

# Подсистема: Cli (L1)

Подсистема `src/cli` с ключевыми точками входа: src/cli/mod.rs

> **Идентификатор:** `src_cli`
> **Теги:** Rust (Cargo), src_cli

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `src/cli/check.rs` | Исходный код |
| `src/cli/config.rs` | Исходный код |
| `src/cli/feedback.rs` | Исходный код |
| `src/cli/init_update.rs` | Исходный код |
| `src/cli/mcp.rs` | Исходный код |
| `src/cli/migrate.rs` | Исходный код |
| `src/cli/mod.rs` | ⭐ Точка входа |
| `src/cli/output.rs` | Исходный код |
| `src/cli/pack.rs` | Исходный код |
| `src/cli/scan.rs` | Исходный код |
| `src/cli/task.rs` | Исходный код |
| `src/cli/telemetry.rs` | Исходный код |
| `src/cli/workspace.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub struct GlobalArgs
pub fn router_for(project: &ResolvedProject) -> Result<RoleRouter>
pub fn run() -> Result<ExitCode>
pub struct CheckArgs
pub fn run(global: &GlobalArgs, args: &CheckArgs) -> Result<ExitCode>
pub struct ConfigArgs
pub enum ConfigCommand
pub fn run(global: &GlobalArgs, args: &ConfigArgs) -> Result<ExitCode>
pub struct FeedbackArgs
pub fn run(global: &GlobalArgs, args: &FeedbackArgs) -> Result<ExitCode>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test
```
