---
directory: src/cli
source:
  kind: git
  blob_sha: 368a10b80d59a10eabe1d3b0fd11cfb79a27b393
  commit_sha: 4f153c6ee310b681808626eb7348ac3ca484e21d
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
pub fn overrides(&self, rtk: Option<RtkMode>) -> Result<CliOverrides>
pub fn load_project(&self, path: &Path, rtk: Option<RtkMode>) -> Result<ResolvedProject>
pub fn router_for(project: &ResolvedProject) -> Result<RoleRouter>
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
pub fn run(global: &GlobalArgs, args: &SearchArgs) -> Result<ExitCode>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test
```
