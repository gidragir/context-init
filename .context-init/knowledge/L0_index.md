# 🗺 Карта архитектуры: context-init (L0)

> **Стек:** Rust (Cargo) | **Обновлено:** 2026-11-07
> **Описание:** CLI-утилита на Rust для автоматической инициализации и поддержания трехуровневой базы знаний (**OpenViking L0-L2**) для автономных ИИ-ассистентов (Antigravity, Hermes, Claude Code, Cursor, Aider).

## 🚪 Точки входа (Root Entrypoints)
- `src/main.rs`
- `Cargo.toml`
- `mise.toml`

## ⚡️ Быстрые команды
```bash
# Сборка
rtk mise run build

# Тесты
rtk mise run test

# Линтер
rtk mise run lint

# Запуск
rtk cargo run

```

## 🧩 Каталог подсистем (L1)

| Подсистема | Роль / Назначение | Точки входа | L1 Карточка |
| :--- | :--- | :--- | :--- |
| **Ci Core** | Подсистема `crates/ci-core` с ключевыми точками входа: crates/ci-core/Cargo.toml, crates/ci-core/src/lib.rs | `crates/ci-core/Cargo.toml, crates/ci-core/src/lib.rs` | [subsystems/crates_ci-core.md](subsystems/crates_ci-core.md) |
| **Ci Core Analysis** | Подсистема `crates/ci-core/src/analysis` с ключевыми точками входа: crates/ci-core/src/analysis/mod.rs | `crates/ci-core/src/analysis/mod.rs` | [subsystems/crates_ci-core_analysis.md](subsystems/crates_ci-core_analysis.md) |
| **Ci Core Analysis Detector** | Модуль и компоненты директории `crates/ci-core/src/analysis/detector` (2 файлов) | `-` | [subsystems/crates_ci-core_analysis_detector.md](subsystems/crates_ci-core_analysis_detector.md) |
| **Ci Core Analysis Detector Gradle** | Модуль-файл `crates/ci-core/src/analysis/detector/gradle.rs` | `-` | [subsystems/crates_ci-core_analysis_detector_gradle.md](subsystems/crates_ci-core_analysis_detector_gradle.md) |
| **Ci Core Analysis Detector Tests** | Тесты для `crates_ci-core_analysis_detector` | `-` | [subsystems/crates_ci-core_analysis_detector_tests.md](subsystems/crates_ci-core_analysis_detector_tests.md) |
| **Ci Core Analysis Detector Tests Classifier** | Модуль-файл `crates/ci-core/src/analysis/detector/tests_classifier.rs` | `-` | [subsystems/crates_ci-core_analysis_detector_tests_classifier.md](subsystems/crates_ci-core_analysis_detector_tests_classifier.md) |
| **Ci Core Analysis Generator** | Модуль-файл `crates/ci-core/src/analysis/generator.rs` | `-` | [subsystems/crates_ci-core_analysis_generator.md](subsystems/crates_ci-core_analysis_generator.md) |
| **Ci Core Analysis Lang** | Подсистема `crates/ci-core/src/analysis/lang` с ключевыми точками входа: crates/ci-core/src/analysis/lang/mod.rs | `crates/ci-core/src/analysis/lang/mod.rs` | [subsystems/crates_ci-core_analysis_lang.md](subsystems/crates_ci-core_analysis_lang.md) |
| **Ci Core Analysis Pack** | Модуль-файл `crates/ci-core/src/analysis/pack.rs` | `-` | [subsystems/crates_ci-core_analysis_pack.md](subsystems/crates_ci-core_analysis_pack.md) |
| **Ci Core Analysis Scan** | Модуль-файл `crates/ci-core/src/analysis/scan.rs` | `-` | [subsystems/crates_ci-core_analysis_scan.md](subsystems/crates_ci-core_analysis_scan.md) |
| **Ci Core Analysis Task** | Модуль-файл `crates/ci-core/src/analysis/task.rs` | `-` | [subsystems/crates_ci-core_analysis_task.md](subsystems/crates_ci-core_analysis_task.md) |
| **Ci Core Config** | Модуль-файл `crates/ci-core/src/config.rs` | `-` | [subsystems/crates_ci-core_config.md](subsystems/crates_ci-core_config.md) |
| **Ci Core Git** | Модуль-файл `crates/ci-core/src/git.rs` | `-` | [subsystems/crates_ci-core_git.md](subsystems/crates_ci-core_git.md) |
| **Ci Core Openviking** | Модуль-файл `crates/ci-core/src/openviking.rs` | `-` | [subsystems/crates_ci-core_openviking.md](subsystems/crates_ci-core_openviking.md) |
| **Ci Core Storage** | Модуль-файл `crates/ci-core/src/storage.rs` | `-` | [subsystems/crates_ci-core_storage.md](subsystems/crates_ci-core_storage.md) |
| **Ci Core Telemetry** | Модуль и компоненты директории `crates/ci-core/src/telemetry` (2 файлов) | `-` | [subsystems/crates_ci-core_telemetry.md](subsystems/crates_ci-core_telemetry.md) |
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
| **Ci Mcp Pipeline** | Модуль-файл `crates/ci-mcp/src/pipeline.rs` | `-` | [subsystems/crates_ci-mcp_pipeline.md](subsystems/crates_ci-mcp_pipeline.md) |
| **Ci Mcp Server** | Модуль-файл `crates/ci-mcp/src/server.rs` | `-` | [subsystems/crates_ci-mcp_server.md](subsystems/crates_ci-mcp_server.md) |
| **Ci Mcp Tools** | Модуль-файл `crates/ci-mcp/src/tools.rs` | `-` | [subsystems/crates_ci-mcp_tools.md](subsystems/crates_ci-mcp_tools.md) |
| **Docs** | Модуль и компоненты директории `docs` (13 файлов) | `-` | [subsystems/docs.md](subsystems/docs.md) |
| **References** | Модуль и компоненты директории `references` (1 файлов) | `-` | [subsystems/references.md](subsystems/references.md) |
| **Reports** | Модуль и компоненты директории `reports` (15 файлов) | `-` | [subsystems/reports.md](subsystems/reports.md) |
| **Scripts** | Модуль и компоненты директории `scripts` (9 файлов) | `-` | [subsystems/scripts.md](subsystems/scripts.md) |
| **Cli** | Подсистема `src/cli` с ключевыми точками входа: src/cli/mod.rs | `src/cli/mod.rs` | [subsystems/src_cli.md](subsystems/src_cli.md) |
| **Cli Setup** | Подсистема `src/cli/setup` с ключевыми точками входа: src/cli/setup/mod.rs | `src/cli/setup/mod.rs` | [subsystems/src_cli_setup.md](subsystems/src_cli_setup.md) |
| **Tests** | Тесты проекта | `-` | [subsystems/tests.md](subsystems/tests.md) |

## 📋 Инварианты и правила проекта
- Все внешние крейты и зависимости строго объявляются в Cargo.toml.
- Код должен проходить `rtk cargo clippy --all-targets` и `rtk cargo fmt --check` без ошибок.
- Использование `unsafe` запрещено без исчерпывающего комментария безопасности `// SAFETY:`.
- Публичные структуры и функции должны сопровождаться docstring (`///`).
