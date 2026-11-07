# ADR 0001: Начальная архитектура и топология context-init

- **Статус:** Принято
- **Дата:** 2026-11-07
- **Стек:** Rust (Cargo)

## Контекст
Инициализация кодовой базы `context-init`. Для обеспечения надежной автономной навигации ИИ-ассистентов зафиксирована трехуровневая модель контекста (OpenViking L0-L2).

## Архитектурные границы подсистем
- **Ci Core** (`crates_ci-core`): Подсистема `crates/ci-core` с ключевыми точками входа: crates/ci-core/Cargo.toml, crates/ci-core/src/lib.rs
- **Ci Core Analysis** (`crates_ci-core_analysis`): Подсистема `crates/ci-core/src/analysis` с ключевыми точками входа: crates/ci-core/src/analysis/mod.rs
- **Ci Core Analysis Detector** (`crates_ci-core_analysis_detector`): Модуль и компоненты директории `crates/ci-core/src/analysis/detector` (2 файлов)
- **Ci Core Analysis Detector Gradle** (`crates_ci-core_analysis_detector_gradle`): Модуль-файл `crates/ci-core/src/analysis/detector/gradle.rs`
- **Ci Core Analysis Detector Tests** (`crates_ci-core_analysis_detector_tests`): Тесты для `crates_ci-core_analysis_detector`
- **Ci Core Analysis Detector Tests Classifier** (`crates_ci-core_analysis_detector_tests_classifier`): Модуль-файл `crates/ci-core/src/analysis/detector/tests_classifier.rs`
- **Ci Core Analysis Generator** (`crates_ci-core_analysis_generator`): Модуль-файл `crates/ci-core/src/analysis/generator.rs`
- **Ci Core Analysis Lang** (`crates_ci-core_analysis_lang`): Подсистема `crates/ci-core/src/analysis/lang` с ключевыми точками входа: crates/ci-core/src/analysis/lang/mod.rs
- **Ci Core Analysis Pack** (`crates_ci-core_analysis_pack`): Модуль-файл `crates/ci-core/src/analysis/pack.rs`
- **Ci Core Analysis Scan** (`crates_ci-core_analysis_scan`): Модуль-файл `crates/ci-core/src/analysis/scan.rs`
- **Ci Core Analysis Task** (`crates_ci-core_analysis_task`): Модуль-файл `crates/ci-core/src/analysis/task.rs`
- **Ci Core Config** (`crates_ci-core_config`): Модуль-файл `crates/ci-core/src/config.rs`
- **Ci Core Git** (`crates_ci-core_git`): Модуль-файл `crates/ci-core/src/git.rs`
- **Ci Core Openviking** (`crates_ci-core_openviking`): Модуль-файл `crates/ci-core/src/openviking.rs`
- **Ci Core Storage** (`crates_ci-core_storage`): Модуль-файл `crates/ci-core/src/storage.rs`
- **Ci Core Telemetry** (`crates_ci-core_telemetry`): Модуль и компоненты директории `crates/ci-core/src/telemetry` (2 файлов)
- **Ci Llm** (`crates_ci-llm`): Подсистема `crates/ci-llm` с ключевыми точками входа: crates/ci-llm/Cargo.toml, crates/ci-llm/src/lib.rs
- **Ci Llm Decorators** (`crates_ci-llm_decorators`): Модуль-файл `crates/ci-llm/src/decorators.rs`
- **Ci Llm Enrich** (`crates_ci-llm_enrich`): Модуль-файл `crates/ci-llm/src/enrich.rs`
- **Ci Llm Prompts** (`crates_ci-llm_prompts`): Модуль-файл `crates/ci-llm/src/prompts.rs`
- **Ci Llm Providers** (`crates_ci-llm_providers`): Модуль-файл `crates/ci-llm/src/providers.rs`
- **Ci Llm Router** (`crates_ci-llm_router`): Модуль-файл `crates/ci-llm/src/router.rs`
- **Ci Llm Schema** (`crates_ci-llm_schema`): Модуль-файл `crates/ci-llm/src/schema.rs`
- **Ci Mcp** (`crates_ci-mcp`): Подсистема `crates/ci-mcp` с ключевыми точками входа: crates/ci-mcp/Cargo.toml, crates/ci-mcp/src/lib.rs
- **Ci Mcp Handlers** (`crates_ci-mcp_handlers`): Модуль-файл `crates/ci-mcp/src/handlers.rs`
- **Ci Mcp Pagination** (`crates_ci-mcp_pagination`): Модуль-файл `crates/ci-mcp/src/pagination.rs`
- **Ci Mcp Pipeline** (`crates_ci-mcp_pipeline`): Модуль-файл `crates/ci-mcp/src/pipeline.rs`
- **Ci Mcp Server** (`crates_ci-mcp_server`): Модуль-файл `crates/ci-mcp/src/server.rs`
- **Ci Mcp Tools** (`crates_ci-mcp_tools`): Модуль-файл `crates/ci-mcp/src/tools.rs`
- **Docs** (`docs`): Модуль и компоненты директории `docs` (13 файлов)
- **References** (`references`): Модуль и компоненты директории `references` (1 файлов)
- **Reports** (`reports`): Модуль и компоненты директории `reports` (15 файлов)
- **Scripts** (`scripts`): Модуль и компоненты директории `scripts` (9 файлов)
- **Cli** (`src_cli`): Подсистема `src/cli` с ключевыми точками входа: src/cli/mod.rs
- **Cli Setup** (`src_cli_setup`): Подсистема `src/cli/setup` с ключевыми точками входа: src/cli/setup/mod.rs
- **Tests** (`tests`): Тесты проекта


## Решение
1. `L0_index.json` и `L0_index.md` служат единой точкой входа (Mental Map) с минимальным потреблением токенов.
2. Каждая подсистема изолирована в карточке L1 (`subsystems/*.md`) с перечислением точек входа и сигнатур.
3. Справочник команд L2 (`cli/commands.md`) фиксирует фактические команды сборки и верификации.

## Последствия
- Устраняется необходимость рекурсивного сканирования всего репозитория при каждом промпте.
- Агенты обращаются к точечным карточкам L1 по мере необходимости.
