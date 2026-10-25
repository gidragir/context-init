# Подсистема: Ci Mcp (L1)

> **Идентификатор:** `crates_ci-mcp`
> **Теги:** Rust (Cargo), crates_ci-mcp

## 🎯 Назначение и роль
Подсистема `crates/ci-mcp` с ключевыми точками входа: crates/ci-mcp/Cargo.toml, crates/ci-mcp/src/lib.rs

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-mcp/Cargo.toml` | ⭐ Точка входа |
| `crates/ci-mcp/src/lib.rs` | ⭐ Точка входа |
| `crates/ci-mcp/src/pipeline.rs` | Исходный код |
| `crates/ci-mcp/src/tools.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub const VERSION: &str = env !("CARGO_PKG_VERSION")
pub fn crate_info() -> String
pub struct PipelineOptions<'a>
pub struct PipelineOutcome
pub fn run_init(project: &ResolvedProject, router: &RoleRouter, opts: &PipelineOptions, ) -> Result<PipelineOutcome>
pub fn run_update(project: &ResolvedProject, router: &RoleRouter, opts: &PipelineOptions, ) -> Result<PipelineOutcome>
pub fn storage_mismatch_hint(project: &ResolvedProject) -> Option<String>
pub struct McpCheckParams
pub struct McpCheckResult
pub struct McpGetSubsystemsParams
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-mcp
```
