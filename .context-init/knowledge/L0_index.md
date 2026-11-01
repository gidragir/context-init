# 🗺 Карта архитектуры: context-init (L0)

> **Стек:** Rust (Cargo) | **Обновлено:** 2026-11-01
> **Описание:** CLI-утилита на Rust для автоматической инициализации и поддержания трехуровневой базы знаний (**OpenViking L0-L2**) для автономных ИИ-ассистентов (Antigravity, Hermes, Claude Code, Cursor, Aider).

## 🚪 Точки входа (Root Entrypoints)
- `src/main.rs`
- `Cargo.toml`
- `mise.toml`

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
| **ci-core** | Ядро без LLM: версия крейта, трейт Retriever и SearchHit для поиска по базе знаний; реэкспорт модулей analysis, config, git, storage, openviking, telemetry. | `crates/ci-core/Cargo.toml, crates/ci-core/src/lib.rs` | [subsystems/crates_ci-core.md](subsystems/crates_ci-core.md) |
| **Анализ проекта** | Сканирование дерева (scan_project, ScanReport), конфиг области (scope), выделение подсистем и задачный дайджест; resolve_knowledge_dir выбирает repo (.context-init/knowledge) или центральное хранилище. | `crates/ci-core/src/analysis/mod.rs` | [subsystems/crates_ci-core_analysis.md](subsystems/crates_ci-core_analysis.md) |
| **Детектор проекта** | analyze_project / analyze_project_with: определение типа проекта, команд, подсистем и сигнатур (deep); список IGNORED_DIRS и префикс rtk для генерируемых команд. | `-` | [subsystems/crates_ci-core_analysis_detector.md](subsystems/crates_ci-core_analysis_detector.md) |
| **Тесты детектора** | Юнит-тесты детектора; символы Engine, Status, MyService — фикстурный Rust-код для проверки извлечения сигнатур, а не API проекта. | `-` | [subsystems/crates_ci-core_analysis_detector_tests.md](subsystems/crates_ci-core_analysis_detector_tests.md) |
| **Генератор базы знаний** | Рендер L0-индекса и OKF-карточек L1 (render_subsystem_card_okf), generate для записи базы, audit для поиска дрейфа, restamp_card_source для обновления commit/blob SHA во frontmatter. | `-` | [subsystems/crates_ci-core_analysis_generator.md](subsystems/crates_ci-core_analysis_generator.md) |
| **Языковые адаптеры** | Трейт LanguageAdapter (language_id, extensions, extract_symbols) и адаптеры Rust, Python, BSL и MDO (1С); ELIDED_BODY — маркер вырезанного тела при сжатии. | `crates/ci-core/src/analysis/lang/mod.rs` | [subsystems/crates_ci-core_analysis_lang.md](subsystems/crates_ci-core_analysis_lang.md) |
| **Упаковщик контекста** | pack_subsystem / pack_files: сборка файлов подсистемы в XML или Markdown с AST-сжатием (тела функций вырезаются) для передачи в LLM. | `-` | [subsystems/crates_ci-core_analysis_pack.md](subsystems/crates_ci-core_analysis_pack.md) |
| **Конфигурация** | Каскад конфигурации (глобальный, проектный, --config, env, CLI): язык (ru по умолчанию), бюджет ответа 4000 токенов, порог разбиения 15000, модели [models], роли [roles] (none = детерминированно), интеграции и телеметрия. | `-` | [subsystems/crates_ci-core_config.md](subsystems/crates_ci-core_config.md) |
| **Git-бэкенд** | Трейт GitBackend (head_commit, статус, diff), GitStatusEntry и StalenessReport для определения устаревания карточек относительно коммитов. | `-` | [subsystems/crates_ci-core_git.md](subsystems/crates_ci-core_git.md) |
| **Формат OpenViking/OKF** | Типы frontmatter OKF-карточек: CardSource, CardGeneratedBy, CardFreshness, CardFrontmatter; лимит L0-аннотации 256 символов. | `-` | [subsystems/crates_ci-core_openviking.md](subsystems/crates_ci-core_openviking.md) |
| **Хранилище** | L0-индекс (схема v3), нормализация путей, переводов строк и git remote; идентификация проекта для центрального хранилища в XDG data dir. | `-` | [subsystems/crates_ci-core_storage.md](subsystems/crates_ci-core_storage.md) |
| **Телеметрия** | Opt-in JSONL-телеметрия без путей и кода: операционные (tool, channel mcp/cli, harness), эффективность (токены ответа vs исходника, усечения, stale), LLM- и сессионные сигналы; рендер дайджеста. | `-` | [subsystems/crates_ci-core_telemetry.md](subsystems/crates_ci-core_telemetry.md) |
| **ci-llm** | Слой LLM: трейт провайдера, RoleName, AgentWorkPackage и ValidatedAgentOutput для передачи работы агенту-харнессу через MCP. | `crates/ci-llm/Cargo.toml, crates/ci-llm/src/lib.rs` | [subsystems/crates_ci-llm.md](subsystems/crates_ci-llm.md) |
| **Декораторы провайдеров** | RetryDecorator с backoff и TimeoutDecorator — обёртки над LlmProvider для повторов и таймаутов. | `-` | [subsystems/crates_ci-llm_decorators.md](subsystems/crates_ci-llm_decorators.md) |
| **Обогащение подсистем** | enrich_subsystems: прогон роли через маршрутизатор и слияние сводок/тегов по id; EnrichOutcome (Skipped, Enriched, PendingAgent, Failed) никогда не ломает конвейер. | `-` | [subsystems/crates_ci-llm_enrich.md](subsystems/crates_ci-llm_enrich.md) |
| **Шаблоны промптов** | Реестр PromptTemplate с источниками (встроенные/пользовательские) и PromptVars (факты, проект, язык) для ролей init/update/task/search. | `-` | [subsystems/crates_ci-llm_prompts.md](subsystems/crates_ci-llm_prompts.md) |
| **LLM-провайдеры** | Провайдеры ollama, command и agent (handoff через context_submit); заглушки openai_compat/anthropic/gemini; парсинг SubsystemAiSummary, healthcheck Ollama, таймаут 60 с по умолчанию. | `-` | [subsystems/crates_ci-llm_providers.md](subsystems/crates_ci-llm_providers.md) |
| **Маршрутизатор ролей** | RoleRouter исполняет цепочку моделей для роли с фолбэком; RoleExecutionOutcome различает детерминированный, завершённый и фолбэк-исход. | `-` | [subsystems/crates_ci-llm_router.md](subsystems/crates_ci-llm_router.md) |
| **JSON-схемы ответов** | Извлечение JSON из ответа модели, валидация по JSON Schema и схемы по умолчанию для каждой роли (default_role_json_schema). | `-` | [subsystems/crates_ci-llm_schema.md](subsystems/crates_ci-llm_schema.md) |
| **ci-mcp** | MCP-сервер context-init: параметры и результаты инструментов (McpCheckParams, McpGetSubsystemsParams и др.) в tools.rs. | `crates/ci-mcp/Cargo.toml, crates/ci-mcp/src/lib.rs` | [subsystems/crates_ci-mcp.md](subsystems/crates_ci-mcp.md) |
| **Обработчики MCP** | ResolvedProject (загрузка конфига и базы), обработчики get_subsystems/get_subsystem/pack/search/task/check/submit; ограничение pack all (5 страниц), сохранение агентских submission с проверкой id. | `-` | [subsystems/crates_ci-mcp_handlers.md](subsystems/crates_ci-mcp_handlers.md) |
| **Пагинация** | Подсчёт токенов, бюджетирование ответа и курсоры base64url (CursorData) для постраничной выдачи больших результатов. | `-` | [subsystems/crates_ci-mcp_pagination.md](subsystems/crates_ci-mcp_pagination.md) |
| **Конвейер init/update** | run_init / run_update: анализ, обогащение ролями, генерация базы и AGENTS.md; FullDriftReport объединяет файловый и git-дрейф. | `-` | [subsystems/crates_ci-mcp_pipeline.md](subsystems/crates_ci-mcp_pipeline.md) |
| **MCP-сервер** | ContextInitServer на stdio: регистрация инструментов, MCP roots (roots/list) для выбора проекта; TelemetrySink подключается через with_telemetry_sink (в продакшен-запуске run_mcp_server не подключён). | `-` | [subsystems/crates_ci-mcp_server.md](subsystems/crates_ci-mcp_server.md) |
| **Ci Mcp Tools** | Модуль-файл `crates/ci-mcp/src/tools.rs` | `-` | [subsystems/crates_ci-mcp_tools.md](subsystems/crates_ci-mcp_tools.md) |
| **Справочники** | Спецификация формата OpenViking (openviking_spec.md) — опорный документ для уровней L0-L2. | `-` | [subsystems/references.md](subsystems/references.md) |
| **Отчёты бенчмарков** | BENCHMARK_REPORT.md и benchmark_latest.json — результаты регрессионного бенчмарка (токены, точность). | `-` | [subsystems/reports.md](subsystems/reports.md) |
| **Отчёты субагентов** | Сводки работы субагентов по крейтам ci-core, ci-llm, ci-mcp, корню и разделению workspace. | `-` | [subsystems/reports_subagents.md](subsystems/reports_subagents.md) |
| **Верификация ci-core** | Отчёт субагента о проверке крейта ci-core. | `-` | [subsystems/reports_subagents_ci-core_verification.md](subsystems/reports_subagents_ci-core_verification.md) |
| **Верификация ci-llm** | Отчёт субагента о проверке крейта ci-llm. | `-` | [subsystems/reports_subagents_ci-llm_verification.md](subsystems/reports_subagents_ci-llm_verification.md) |
| **Верификация ci-mcp** | Отчёт субагента о проверке крейта ci-mcp. | `-` | [subsystems/reports_subagents_ci-mcp_verification.md](subsystems/reports_subagents_ci-mcp_verification.md) |
| **Верификация context-init** | Отчёт субагента о проверке корневого бинарного крейта context-init. | `-` | [subsystems/reports_subagents_context-init_verification.md](subsystems/reports_subagents_context-init_verification.md) |
| **Верификация workspace (шаг 1)** | Отчёт субагента о проверке разделения на Cargo workspace (шаг 1 v0.3). | `-` | [subsystems/reports_subagents_step1_workspace_verification.md](subsystems/reports_subagents_step1_workspace_verification.md) |
| **Scripts** | Модуль и компоненты директории `scripts` (8 файлов) | `-` | [subsystems/scripts.md](subsystems/scripts.md) |
| **CLI** | Точка входа run() и подкоманды init, update, check, pack, mcp, config, migrate, setup, telemetry, feedback; GlobalArgs (--model, --role, --config, --compact) и сборка RoleRouter. | `src/cli/mod.rs` | [subsystems/src_cli.md](subsystems/src_cli.md) |
| **CLI setup** | context-init setup: план регистрации MCP-сервера и короткого правила агента для харнессов (Claude Code, Antigravity и др.), слияние с существующими конфигами. | `src/cli/setup/mod.rs` | [subsystems/src_cli_setup.md](subsystems/src_cli_setup.md) |
| **Интеграционные тесты CLI** | tests/cli.rs — сквозные тесты бинаря context-init по подкомандам. | `-` | [subsystems/tests.md](subsystems/tests.md) |
| **Tests Cli** | Тесты для `tests` | `-` | [subsystems/tests_cli.md](subsystems/tests_cli.md) |

## 📋 Инварианты и правила проекта
- Все внешние крейты и зависимости строго объявляются в Cargo.toml.
- Код должен проходить `rtk cargo clippy --all-targets` и `rtk cargo fmt --check` без ошибок.
- Использование `unsafe` запрещено без исчерпывающего комментария безопасности `// SAFETY:`.
- Публичные структуры и функции должны сопровождаться docstring (`///`).
