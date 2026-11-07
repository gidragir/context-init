# Инструкции для ИИ-агентов в проекте context-init

Данный проект настроен для автономной работы ассистентов.

## 🗺 Архитектурная карта и навигация (OpenViking L0-L2)

Проект `context-init` описан трехуровневой базой знаний context-init (L0-L2), которая лежит в `.context-init/knowledge/` и коммитится вместе с кодом. Людям можно читать её напрямую; агентам — предпочитать MCP/`pack` ниже: это компактнее и учитывает бюджет токенов. После структурных изменений запустите `context-init update` и закоммитьте изменения базы в том же PR.

### ⚡ MCP-First Protocol (context-init)

1. `context_get_subsystems` — карта подсистем проекта (L0).
2. `context_get_subsystem(subsystem_id)` — компактная L1-карточка подсистемы.
3. `context_pack(subsystem_id, compress=true)` — AST-сжатый код подсистемы.
4. `context_check` — проверка архитектурного дрейфа перед сдачей задачи.

Без доступа к MCP (субагенты, скрипты): `rtk context-init pack <subsystem_id> --compress` и `rtk context-init check --compact`.

### 📦 Доступные подсистемы:
- `crates_ci-core` (Ci Core) — Подсистема `crates/ci-core` с ключевыми точками входа: crates/ci-core/Cargo.toml, crates/ci-core/src/lib.rs
- `crates_ci-core_analysis` (Ci Core Analysis) — Подсистема `crates/ci-core/src/analysis` с ключевыми точками входа: crates/ci-core/src/analysis/mod.rs
- `crates_ci-core_analysis_detector` (Ci Core Analysis Detector) — Модуль и компоненты директории `crates/ci-core/src/analysis/detector` (2 файлов)
- `crates_ci-core_analysis_detector_gradle` (Ci Core Analysis Detector Gradle) — Модуль-файл `crates/ci-core/src/analysis/detector/gradle.rs`
- `crates_ci-core_analysis_detector_tests` (Ci Core Analysis Detector Tests) — Тесты для `crates_ci-core_analysis_detector`
- `crates_ci-core_analysis_detector_tests_classifier` (Ci Core Analysis Detector Tests Classifier) — Модуль-файл `crates/ci-core/src/analysis/detector/tests_classifier.rs`
- `crates_ci-core_analysis_generator` (Ci Core Analysis Generator) — Модуль-файл `crates/ci-core/src/analysis/generator.rs`
- `crates_ci-core_analysis_lang` (Ci Core Analysis Lang) — Подсистема `crates/ci-core/src/analysis/lang` с ключевыми точками входа: crates/ci-core/src/analysis/lang/mod.rs
- `crates_ci-core_analysis_pack` (Ci Core Analysis Pack) — Модуль-файл `crates/ci-core/src/analysis/pack.rs`
- `crates_ci-core_analysis_scan` (Ci Core Analysis Scan) — Модуль-файл `crates/ci-core/src/analysis/scan.rs`
- `crates_ci-core_analysis_task` (Ci Core Analysis Task) — Модуль-файл `crates/ci-core/src/analysis/task.rs`
- `crates_ci-core_config` (Ci Core Config) — Модуль-файл `crates/ci-core/src/config.rs`
- `crates_ci-core_git` (Ci Core Git) — Модуль-файл `crates/ci-core/src/git.rs`
- `crates_ci-core_openviking` (Ci Core Openviking) — Модуль-файл `crates/ci-core/src/openviking.rs`
- `crates_ci-core_storage` (Ci Core Storage) — Модуль-файл `crates/ci-core/src/storage.rs`
- `crates_ci-core_telemetry` (Ci Core Telemetry) — Модуль и компоненты директории `crates/ci-core/src/telemetry` (2 файлов)
- `crates_ci-llm` (Ci Llm) — Подсистема `crates/ci-llm` с ключевыми точками входа: crates/ci-llm/Cargo.toml, crates/ci-llm/src/lib.rs
- `crates_ci-llm_decorators` (Ci Llm Decorators) — Модуль-файл `crates/ci-llm/src/decorators.rs`
- `crates_ci-llm_enrich` (Ci Llm Enrich) — Модуль-файл `crates/ci-llm/src/enrich.rs`
- `crates_ci-llm_prompts` (Ci Llm Prompts) — Модуль-файл `crates/ci-llm/src/prompts.rs`
- `crates_ci-llm_providers` (Ci Llm Providers) — Модуль-файл `crates/ci-llm/src/providers.rs`
- `crates_ci-llm_router` (Ci Llm Router) — Модуль-файл `crates/ci-llm/src/router.rs`
- `crates_ci-llm_schema` (Ci Llm Schema) — Модуль-файл `crates/ci-llm/src/schema.rs`
- `crates_ci-mcp` (Ci Mcp) — Подсистема `crates/ci-mcp` с ключевыми точками входа: crates/ci-mcp/Cargo.toml, crates/ci-mcp/src/lib.rs
- `crates_ci-mcp_handlers` (Ci Mcp Handlers) — Модуль-файл `crates/ci-mcp/src/handlers.rs`
- `crates_ci-mcp_pagination` (Ci Mcp Pagination) — Модуль-файл `crates/ci-mcp/src/pagination.rs`
- `crates_ci-mcp_pipeline` (Ci Mcp Pipeline) — Модуль-файл `crates/ci-mcp/src/pipeline.rs`
- `crates_ci-mcp_server` (Ci Mcp Server) — Модуль-файл `crates/ci-mcp/src/server.rs`
- `crates_ci-mcp_tools` (Ci Mcp Tools) — Модуль-файл `crates/ci-mcp/src/tools.rs`
- `docs` (Docs) — Модуль и компоненты директории `docs` (13 файлов)
- `references` (References) — Модуль и компоненты директории `references` (1 файлов)
- `reports` (Reports) — Модуль и компоненты директории `reports` (15 файлов)
- `scripts` (Scripts) — Модуль и компоненты директории `scripts` (9 файлов)
- `src_cli` (Cli) — Подсистема `src/cli` с ключевыми точками входа: src/cli/mod.rs
- `src_cli_setup` (Cli Setup) — Подсистема `src/cli/setup` с ключевыми точками входа: src/cli/setup/mod.rs
- `tests` (Tests) — Тесты проекта
