---
directory: src/cli
source:
  kind: git
  blob_sha: 341e7a5b025e537093e70f98e72133e11675dbd8
  commit_sha: e9daa96fc49222863d0ca3be1c21d8edee9a512b
generated_by:
  component: context-init
  role: init
  model: none
freshness:
  total_entries: 14
  sampled_entries: 14
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
| `src/cli/search.rs` | Исходный код |
| `src/cli/task.rs` | Исходный код |
| `src/cli/telemetry.rs` | Исходный код |
| `src/cli/workspace.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub struct GlobalArgs
pub fn router_for(project: &ResolvedProject) -> Result<RoleRouter>
pub fn telemetry_scope(project: &ResolvedProject) -> TelemetryScope
pub fn run() -> Result<ExitCode>
pub struct CheckArgs
pub fn run(global: &GlobalArgs, args: &CheckArgs) -> Result<ExitCode>
pub struct ConfigArgs
pub enum ConfigCommand
pub fn run(global: &GlobalArgs, args: &ConfigArgs) -> Result<ExitCode>
pub struct FeedbackArgs
```

## 🧪 Запуск и тестирование
```bash
rtk mise run test
```
