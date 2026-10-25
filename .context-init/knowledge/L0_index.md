# 🗺 Карта архитектуры: context-init (L0)

> **Стек:** Rust (Cargo) | **Обновлено:** 2026-10-25
> **Описание:** CLI-утилита на Rust для автоматической инициализации и поддержания трехуровневой базы знаний (**OpenViking L0-L2**) для автономных ИИ-ассистентов (Antigravity, Hermes, Claude Code, Cursor, Aider).

## 🚪 Точки входа (Root Entrypoints)
- `src/main.rs`
- `Cargo.toml`

## ⚡️ Быстрые команды
```bash
# Сборка
rtk cargo build

# Тесты
rtk cargo test

# Линтер
rtk cargo clippy

# Запуск
rtk cargo run

```

## 🧩 Каталог подсистем (L1)

| Подсистема | Роль / Назначение | Точки входа | L1 Карточка |
| :--- | :--- | :--- | :--- |
| **Ci Core** | Подсистема `crates/ci-core` с ключевыми точками входа: crates/ci-core/Cargo.toml, crates/ci-core/src/lib.rs | `crates/ci-core/Cargo.toml, crates/ci-core/src/lib.rs` | [subsystems/crates_ci-core.md](subsystems/crates_ci-core.md) |
| **Ci Core Analysis** | Подсистема `crates/ci-core/src/analysis` с ключевыми точками входа: crates/ci-core/src/analysis/mod.rs | `crates/ci-core/src/analysis/mod.rs` | [subsystems/crates_ci-core_analysis.md](subsystems/crates_ci-core_analysis.md) |
| **Ci Core Analysis Detector** | Модуль и компоненты директории `crates/ci-core/src/analysis/detector` (1 файлов) | `-` | [subsystems/crates_ci-core_analysis_detector.md](subsystems/crates_ci-core_analysis_detector.md) |
| **Ci Core Analysis Detector Tests** | Модуль-файл `crates/ci-core/src/analysis/detector/tests.rs` | `-` | [subsystems/crates_ci-core_analysis_detector_tests.md](subsystems/crates_ci-core_analysis_detector_tests.md) |
| **Ci Core Analysis Generator** | Модуль-файл `crates/ci-core/src/analysis/generator.rs` | `-` | [subsystems/crates_ci-core_analysis_generator.md](subsystems/crates_ci-core_analysis_generator.md) |
| **Ci Core Config** | Модуль-файл `crates/ci-core/src/config.rs` | `-` | [subsystems/crates_ci-core_config.md](subsystems/crates_ci-core_config.md) |
| **Ci Core Openviking** | Модуль-файл `crates/ci-core/src/openviking.rs` | `-` | [subsystems/crates_ci-core_openviking.md](subsystems/crates_ci-core_openviking.md) |
| **Ci Core Storage** | Модуль-файл `crates/ci-core/src/storage.rs` | `-` | [subsystems/crates_ci-core_storage.md](subsystems/crates_ci-core_storage.md) |
| **Ci Core Telemetry** | Модуль-файл `crates/ci-core/src/telemetry.rs` | `-` | [subsystems/crates_ci-core_telemetry.md](subsystems/crates_ci-core_telemetry.md) |
| **Ci Llm** | Подсистема `crates/ci-llm` с ключевыми точками входа: crates/ci-llm/Cargo.toml, crates/ci-llm/src/lib.rs | `crates/ci-llm/Cargo.toml, crates/ci-llm/src/lib.rs` | [subsystems/crates_ci-llm.md](subsystems/crates_ci-llm.md) |
| **Ci Llm Decorators** | Модуль-файл `crates/ci-llm/src/decorators.rs` | `-` | [subsystems/crates_ci-llm_decorators.md](subsystems/crates_ci-llm_decorators.md) |
| **Ci Llm Enrich** | Модуль-файл `crates/ci-llm/src/enrich.rs` | `-` | [subsystems/crates_ci-llm_enrich.md](subsystems/crates_ci-llm_enrich.md) |
| **Ci Llm Prompts** | Модуль-файл `crates/ci-llm/src/prompts.rs` | `-` | [subsystems/crates_ci-llm_prompts.md](subsystems/crates_ci-llm_prompts.md) |
| **Ci Llm Providers** | Модуль-файл `crates/ci-llm/src/providers.rs` | `-` | [subsystems/crates_ci-llm_providers.md](subsystems/crates_ci-llm_providers.md) |
| **Ci Llm Router** | Модуль-файл `crates/ci-llm/src/router.rs` | `-` | [subsystems/crates_ci-llm_router.md](subsystems/crates_ci-llm_router.md) |
| **Ci Llm Schema** | Модуль-файл `crates/ci-llm/src/schema.rs` | `-` | [subsystems/crates_ci-llm_schema.md](subsystems/crates_ci-llm_schema.md) |
| **Ci Mcp** | Подсистема `crates/ci-mcp` с ключевыми точками входа: crates/ci-mcp/Cargo.toml, crates/ci-mcp/src/lib.rs | `crates/ci-mcp/Cargo.toml, crates/ci-mcp/src/lib.rs` | [subsystems/crates_ci-mcp.md](subsystems/crates_ci-mcp.md) |
| **Ci Mcp Handlers** | Модуль-файл `crates/ci-mcp/src/handlers.rs` | `-` | [subsystems/crates_ci-mcp_handlers.md](subsystems/crates_ci-mcp_handlers.md) |
| **Ci Mcp Pagination** | Модуль-файл `crates/ci-mcp/src/pagination.rs` | `-` | [subsystems/crates_ci-mcp_pagination.md](subsystems/crates_ci-mcp_pagination.md) |
| **Ci Mcp Server** | Модуль-файл `crates/ci-mcp/src/server.rs` | `-` | [subsystems/crates_ci-mcp_server.md](subsystems/crates_ci-mcp_server.md) |
| **References** | Модуль и компоненты директории `references` (1 файлов) | `-` | [subsystems/references.md](subsystems/references.md) |
| **Reports** | Модуль и компоненты директории `reports` (2 файлов) | `-` | [subsystems/reports.md](subsystems/reports.md) |
| **Reports Subagents** | Модуль и компоненты директории `reports/subagents` (7 файлов) | `-` | [subsystems/reports_subagents.md](subsystems/reports_subagents.md) |
| **Reports Subagents Ci Core Verification** | Модуль-файл `reports/subagents/ci-core_verification.md` | `-` | [subsystems/reports_subagents_ci-core_verification.md](subsystems/reports_subagents_ci-core_verification.md) |
| **Reports Subagents Ci Llm Verification** | Модуль-файл `reports/subagents/ci-llm_verification.md` | `-` | [subsystems/reports_subagents_ci-llm_verification.md](subsystems/reports_subagents_ci-llm_verification.md) |
| **Reports Subagents Ci Mcp Verification** | Модуль-файл `reports/subagents/ci-mcp_verification.md` | `-` | [subsystems/reports_subagents_ci-mcp_verification.md](subsystems/reports_subagents_ci-mcp_verification.md) |
| **Reports Subagents Context Init Verification** | Модуль-файл `reports/subagents/context-init_verification.md` | `-` | [subsystems/reports_subagents_context-init_verification.md](subsystems/reports_subagents_context-init_verification.md) |
| **Reports Subagents Step1 Workspace Verification** | Модуль-файл `reports/subagents/step1_workspace_verification.md` | `-` | [subsystems/reports_subagents_step1_workspace_verification.md](subsystems/reports_subagents_step1_workspace_verification.md) |
| **Cli** | Подсистема `src/cli` с ключевыми точками входа: src/cli/mod.rs | `src/cli/mod.rs` | [subsystems/src_cli.md](subsystems/src_cli.md) |
| **Cli Setup** | Подсистема `src/cli/setup` с ключевыми точками входа: src/cli/setup/mod.rs | `src/cli/setup/mod.rs` | [subsystems/src_cli_setup.md](subsystems/src_cli_setup.md) |
| **Tests** | Модуль и компоненты директории `tests` (2 файлов) | `-` | [subsystems/tests.md](subsystems/tests.md) |

## 📋 Инварианты и правила проекта
- Все внешние крейты и зависимости строго объявляются в Cargo.toml.
- Код должен проходить `rtk cargo clippy --all-targets` и `rtk cargo fmt --check` без ошибок.
- Использование `unsafe` запрещено без исчерпывающего комментария безопасности `// SAFETY:`.
- Публичные структуры и функции должны сопровождаться docstring (`///`).
