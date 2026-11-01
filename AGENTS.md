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
- `crates_ci-core` (ci-core) — Ядро без LLM: версия крейта, трейт Retriever и SearchHit для поиска по базе знаний; реэкспорт модулей analysis, config, git, storage, openviking, telemetry.
- `crates_ci-core_analysis` (Анализ проекта) — Сканирование дерева (scan_project, ScanReport), конфиг области (scope), выделение подсистем и задачный дайджест; resolve_knowledge_dir выбирает repo (.context-init/knowledge) или центральное хранилище.
- `crates_ci-core_analysis_detector` (Детектор проекта) — analyze_project / analyze_project_with: определение типа проекта, команд, подсистем и сигнатур (deep); список IGNORED_DIRS и префикс rtk для генерируемых команд.
- `crates_ci-core_analysis_detector_tests` (Тесты детектора) — Юнит-тесты детектора; символы Engine, Status, MyService — фикстурный Rust-код для проверки извлечения сигнатур, а не API проекта.
- `crates_ci-core_analysis_generator` (Генератор базы знаний) — Рендер L0-индекса и OKF-карточек L1 (render_subsystem_card_okf), generate для записи базы, audit для поиска дрейфа, restamp_card_source для обновления commit/blob SHA во frontmatter.
- `crates_ci-core_analysis_lang` (Языковые адаптеры) — Трейт LanguageAdapter (language_id, extensions, extract_symbols) и адаптеры Rust, Python, BSL и MDO (1С); ELIDED_BODY — маркер вырезанного тела при сжатии.
- `crates_ci-core_analysis_pack` (Упаковщик контекста) — pack_subsystem / pack_files: сборка файлов подсистемы в XML или Markdown с AST-сжатием (тела функций вырезаются) для передачи в LLM.
- `crates_ci-core_config` (Конфигурация) — Каскад конфигурации (глобальный, проектный, --config, env, CLI): язык (ru по умолчанию), бюджет ответа 4000 токенов, порог разбиения 15000, модели [models], роли [roles] (none = детерминированно), интеграции и телеметрия.
- `crates_ci-core_git` (Git-бэкенд) — Трейт GitBackend (head_commit, статус, diff), GitStatusEntry и StalenessReport для определения устаревания карточек относительно коммитов.
- `crates_ci-core_openviking` (Формат OpenViking/OKF) — Типы frontmatter OKF-карточек: CardSource, CardGeneratedBy, CardFreshness, CardFrontmatter; лимит L0-аннотации 256 символов.
- `crates_ci-core_storage` (Хранилище) — L0-индекс (схема v3), нормализация путей, переводов строк и git remote; идентификация проекта для центрального хранилища в XDG data dir.
- `crates_ci-core_telemetry` (Телеметрия) — Opt-in JSONL-телеметрия без путей и кода: операционные (tool, channel mcp/cli, harness), эффективность (токены ответа vs исходника, усечения, stale), LLM- и сессионные сигналы; рендер дайджеста.
- `crates_ci-llm` (ci-llm) — Слой LLM: трейт провайдера, RoleName, AgentWorkPackage и ValidatedAgentOutput для передачи работы агенту-харнессу через MCP.
- `crates_ci-llm_decorators` (Декораторы провайдеров) — RetryDecorator с backoff и TimeoutDecorator — обёртки над LlmProvider для повторов и таймаутов.
- `crates_ci-llm_enrich` (Обогащение подсистем) — enrich_subsystems: прогон роли через маршрутизатор и слияние сводок/тегов по id; EnrichOutcome (Skipped, Enriched, PendingAgent, Failed) никогда не ломает конвейер.
- `crates_ci-llm_prompts` (Шаблоны промптов) — Реестр PromptTemplate с источниками (встроенные/пользовательские) и PromptVars (факты, проект, язык) для ролей init/update/task/search.
- `crates_ci-llm_providers` (LLM-провайдеры) — Провайдеры ollama, command и agent (handoff через context_submit); заглушки openai_compat/anthropic/gemini; парсинг SubsystemAiSummary, healthcheck Ollama, таймаут 60 с по умолчанию.
- `crates_ci-llm_router` (Маршрутизатор ролей) — RoleRouter исполняет цепочку моделей для роли с фолбэком; RoleExecutionOutcome различает детерминированный, завершённый и фолбэк-исход.
- `crates_ci-llm_schema` (JSON-схемы ответов) — Извлечение JSON из ответа модели, валидация по JSON Schema и схемы по умолчанию для каждой роли (default_role_json_schema).
- `crates_ci-mcp` (ci-mcp) — MCP-сервер context-init: параметры и результаты инструментов (McpCheckParams, McpGetSubsystemsParams и др.) в tools.rs.
- `crates_ci-mcp_handlers` (Обработчики MCP) — ResolvedProject (загрузка конфига и базы), обработчики get_subsystems/get_subsystem/pack/search/task/check/submit; ограничение pack all (5 страниц), сохранение агентских submission с проверкой id.
- `crates_ci-mcp_pagination` (Пагинация) — Подсчёт токенов, бюджетирование ответа и курсоры base64url (CursorData) для постраничной выдачи больших результатов.
- `crates_ci-mcp_pipeline` (Конвейер init/update) — run_init / run_update: анализ, обогащение ролями, генерация базы и AGENTS.md; FullDriftReport объединяет файловый и git-дрейф.
- `crates_ci-mcp_server` (MCP-сервер) — ContextInitServer на stdio: регистрация инструментов, MCP roots (roots/list) для выбора проекта; TelemetrySink подключается через with_telemetry_sink (в продакшен-запуске run_mcp_server не подключён).
- `crates_ci-mcp_tools` (Ci Mcp Tools) — Модуль-файл `crates/ci-mcp/src/tools.rs`
- `references` (Справочники) — Спецификация формата OpenViking (openviking_spec.md) — опорный документ для уровней L0-L2.
- `reports` (Отчёты бенчмарков) — BENCHMARK_REPORT.md и benchmark_latest.json — результаты регрессионного бенчмарка (токены, точность).
- `reports_subagents` (Отчёты субагентов) — Сводки работы субагентов по крейтам ci-core, ci-llm, ci-mcp, корню и разделению workspace.
- `reports_subagents_ci-core_verification` (Верификация ci-core) — Отчёт субагента о проверке крейта ci-core.
- `reports_subagents_ci-llm_verification` (Верификация ci-llm) — Отчёт субагента о проверке крейта ci-llm.
- `reports_subagents_ci-mcp_verification` (Верификация ci-mcp) — Отчёт субагента о проверке крейта ci-mcp.
- `reports_subagents_context-init_verification` (Верификация context-init) — Отчёт субагента о проверке корневого бинарного крейта context-init.
- `reports_subagents_step1_workspace_verification` (Верификация workspace (шаг 1)) — Отчёт субагента о проверке разделения на Cargo workspace (шаг 1 v0.3).
- `scripts` (Scripts) — Модуль и компоненты директории `scripts` (8 файлов)
- `src_cli` (CLI) — Точка входа run() и подкоманды init, update, check, pack, mcp, config, migrate, setup, telemetry, feedback; GlobalArgs (--model, --role, --config, --compact) и сборка RoleRouter.
- `src_cli_setup` (CLI setup) — context-init setup: план регистрации MCP-сервера и короткого правила агента для харнессов (Claude Code, Antigravity и др.), слияние с существующими конфигами.
- `tests` (Интеграционные тесты CLI) — tests/cli.rs — сквозные тесты бинаря context-init по подкомандам.
- `tests_cli` (Tests Cli) — Тесты для `tests`
