---
directory: src/cli
source:
  kind: git
  blob_sha: ceaaf1fb5b1d173146d06133e765886a1eacd707
  commit_sha: fff7dd106863757218c552cb8ef5b18e46c70899
generated_by:
  component: context-init
  role: update
  model: none
freshness:
  total_entries: 14
  sampled_entries: 14
  unsampled_entries: 0
  pending_child_changes: 0
---

# Подсистема: CLI (L1)

Точка входа run() и подкоманды init, update, check, pack, mcp, config, migrate, setup, telemetry, feedback; GlobalArgs (--model, --role, --config, --compact) и сборка RoleRouter.

> **Идентификатор:** `src_cli`
> **Теги:** clap, cli, entrypoint

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
pub fn overrides(&self, rtk: Option<RtkMode>) -> Result<CliOverrides>
pub fn load_project(&self, path: &Path, rtk: Option<RtkMode>) -> Result<ResolvedProject>
pub fn router_for(project: &ResolvedProject) -> Result<RoleRouter>
pub fn telemetry_scope(project: &ResolvedProject) -> TelemetryScope
pub fn run() -> Result<ExitCode>
pub struct CheckArgs
pub fn run(global: &GlobalArgs, args: &CheckArgs) -> Result<ExitCode>
pub struct ConfigArgs
pub enum ConfigCommand
pub fn run(global: &GlobalArgs, args: &ConfigArgs) -> Result<ExitCode>
pub struct FeedbackArgs
pub fn run(global: &GlobalArgs, args: &FeedbackArgs) -> Result<ExitCode>
pub struct RtkArgs
pub struct InitArgs
pub struct UpdateArgs
pub fn run_init(global: &GlobalArgs, args: &InitArgs) -> Result<ExitCode>
pub fn run_update(global: &GlobalArgs, args: &UpdateArgs) -> Result<ExitCode>
pub struct McpArgs
pub fn run(args: &McpArgs) -> Result<ExitCode>
pub struct MigrateArgs
pub fn run(global: &GlobalArgs, args: &MigrateArgs) -> Result<ExitCode>
pub fn banner(subtitle: &str)
pub fn analysis_summary(analysis: &ProjectAnalysis)
pub fn notice(msg: &str)
pub struct PackArgs
pub fn run(global: &GlobalArgs, args: &PackArgs) -> Result<ExitCode>
pub struct ScanArgs
pub fn run(global: &GlobalArgs, args: &ScanArgs) -> Result<ExitCode>
pub struct SearchArgs
```

## 🧪 Запуск и тестирование
```bash
cargo test
```
