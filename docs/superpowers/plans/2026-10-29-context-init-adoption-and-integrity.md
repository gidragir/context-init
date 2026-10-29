# context-init: наблюдаемость, целостность конвейера и использование агентами — план реализации

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** каждый вызов context-init измеряется (харнесс, канал, экономия токенов), `context_submit` даёт согласованную базу знаний, рассинхрон бинаря обнаруживается, тестовые фикстуры не загрязняют базу, агенты получают маршрутизирующие подсказки.

**Architecture:** узкий фасад `Recorder` в `ci-core::telemetry` + обёртки вызовов в MCP-сервере и CLI; происхождение сводок (`summary_source`) в L0 v4 и единая функция `generator::apply_summaries`, через которую идут submit и LLM-обогащение; build-SHA из `build.rs`; скрипты аудита/проб в `scripts/` вне продукта.

**Tech Stack:** Rust workspace (`ci-core`, `ci-llm`, `ci-mcp`, корневой `context-init`), rmcp, tantivy (BM25), serde/JSONL, cargo-nextest, mise; Python 3 stdlib для `scripts/`.

**Spec:** `docs/superpowers/specs/2026-10-29-context-init-adoption-and-integrity-design.md`

## Global Constraints

- Все шелл-команды — с префиксом `rtk` (`rtk cargo ...`, `rtk mise run ...`); таск-раннер только `mise.toml`, Makefile запрещён.
- Тесты: `rtk cargo nextest run --workspace` (под `cargo test` `ci-mcp` флакает из-за `XDG_DATA_HOME`); каждый тест, пишущий в data dir, изолирует `XDG_DATA_HOME`/`HOME` во временный каталог.
- Перед коммитом: `rtk cargo clippy --workspace --all-targets -- -D warnings` и `rtk cargo fmt --all` без изменений.
- Коммиты — Conventional Commits, тело заканчивается `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`; сообщение проверяется глазами (oco генерирует не-конвенциональные).
- Телеметрия: без путей, исходного кода, сырых `clientInfo.name` и значений переменных окружения; ошибки записи телеметрии никогда не ломают вызов.
- `HarnessId` значения ровно: `claude-code`, `antigravity`, `cursor`, `codex`, `gemini-cli`, `terminal`, `other`.
- `ErrorClass` значения ровно: `not_found`, `invalid_params`, `io`, `drift`, `internal`.
- `TELEMETRY_SCHEMA_VERSION = 2`, чтение v1 сохраняется; `CURRENT_L0_SCHEMA_VERSION = 4`, чтение v3 с миграцией.
- `SummarySource` значения ровно: `heuristic`, `llm`, `agent`, `manual`.
- Футер рассинхрона дословно: `⚠ context-init обновлён на диске — перезапустите MCP-сервер`.
- Множитель скора для подсистем с тегом `tests`: `0.5`.
- Описание MCP-инструмента ≤ 300 символов.
- Язык генерируемой базы — `ru` (по конфигурации), идентификаторы и комментарии кода — английские, как в окружающем коде.
- После структурных изменений: `rtk cargo build --release && rtk mise run install` (после Task 11), затем `context-init update` и коммит базы в том же PR.

## Review Focus

1. Каталог телеметрии недоступен на запись (нет прав, диск полон) — вызов инструмента обязан вернуть обычный результат; тест в Task 1.
2. Клиент MCP без `clientInfo` или с неизвестным именем — `harness = other`, сервер работает; тест в Task 2.
3. Телеметрия включена глобально, но выключена в `.context-init.toml` разрешённого проекта (и наоборот) — решение по конфигу проекта, разрешённого для вызова; тест в Task 2.
4. База v3 с ручной сводкой подсистемы — после загрузки и `update` сводка сохранена как `manual`, карточка не перезаписана; тест в Task 7.
5. `context_submit` с одним невалидным id среди валидных или сбой записи посреди применения — индекс и карточки не меняются; тест в Task 9.

---

## Карта файлов

| Файл | Ответственность | Задачи |
|---|---|---|
| `crates/ci-core/src/telemetry.rs` | `Recorder`, `CallContext`, `HarnessId`, `Channel`, `CallOutcome`, `ErrorClass`, схема v2, отчёт | 1, 5 |
| `crates/ci-core/src/telemetry/harness.rs` (новый) | нормализация `clientInfo.name` и детект по окружению | 1, 4 |
| `crates/ci-mcp/src/server.rs` | sink, `clientInfo`, `instrumented`, футер рассинхрона, описания инструментов | 2, 11, 14 |
| `crates/ci-mcp/src/handlers.rs` | эффективность, `persist_submission` → `apply_summaries` | 3, 9 |
| `src/cli/mod.rs` | обёртка CLI-команд | 4 |
| `crates/ci-core/src/storage.rs` | L0 v4, миграция v3, `generator` stamp | 7 |
| `crates/ci-core/build.rs` (новый), `crates/ci-core/src/lib.rs` | `ci_core::BUILD` | 7 |
| `crates/ci-core/src/analysis/generator.rs` | рендер из индекса, слияние по `summary_source`, `apply_summaries`, дрейф `format` | 8, 11 |
| `crates/ci-llm/src/schema.rs`, `crates/ci-llm/src/enrich.rs`, `crates/ci-mcp/src/pipeline.rs` | схема update, обогащение только изменённых | 9, 10 |
| `crates/ci-core/src/analysis/detector.rs`, `.../subsystems.rs` | тестовые файлы без символов | 12 |
| `crates/ci-core/src/search.rs` | понижение `tests` | 13 |
| `src/cli/setup/rule_template.md`, `src/cli/setup/rules.rs` | правило в два раздела | 15 |
| `mise.toml`, `README.md` | задача `install` | 11 |
| `scripts/audit-sessions.py`, `scripts/probe-adoption.sh`, `scripts/probes/*.md` | инструменты разработчика | 6, 16 |

---

## Этап A — наблюдаемость

### Task 1: `Recorder` и типы контекста вызова

**Files:**
- Modify: `crates/ci-core/src/telemetry.rs` (типы, `OperationalSignal.nested`, `TELEMETRY_SCHEMA_VERSION = 2`)
- Create: `crates/ci-core/src/telemetry/harness.rs` (перевести `telemetry.rs` в `telemetry/mod.rs`, если модуль нужен подмодулем)
- Modify: `crates/ci-core/src/lib.rs` (реэкспорт)
- Test: тесты внутри `telemetry` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Produces:
  - `pub enum HarnessId { ClaudeCode, Antigravity, Cursor, Codex, GeminiCli, Terminal, Other }` — serde `kebab-case`; `fn as_str(&self) -> &'static str`.
  - `pub fn harness_from_client_name(name: Option<&str>) -> HarnessId` — подстроки без учёта регистра: `claude` → ClaudeCode, `antigravity`/`jetski`/`agy` → Antigravity, `cursor`, `codex`, `gemini` → GeminiCli; `None`/прочее → Other.
  - `pub fn harness_from_env(vars: impl Fn(&str) -> bool) -> HarnessId` — `vars("CLAUDECODE")` → ClaudeCode; маркеры Antigravity — константа `ANTIGRAVITY_ENV_MARKERS: &[&str]` (заполняется в Task 4); иначе Terminal.
  - `pub enum Channel { Mcp, Cli }`, `pub struct CallContext { pub harness: HarnessId, pub channel: Channel, pub nested: bool }`.
  - `pub enum ErrorClass { NotFound, InvalidParams, Io, Drift, Internal }` (serde `snake_case`), `pub enum CallOutcome { Ok, Err(ErrorClass) }`.
  - `pub fn classify_error(err: &anyhow::Error) -> ErrorClass` — `io::Error` в цепочке → Io; сообщение содержит `not found`/`Unknown subsystem` → NotFound; `invalid`/`must` → InvalidParams; `drift` → Drift; иначе Internal.
  - `pub struct Recorder` (`Clone`), `Recorder::new(sink: Option<Arc<dyn TelemetrySink>>, project_id: Option<String>) -> Self`, `Recorder::disabled() -> Self`, `record_call(&self, ctx: &CallContext, tool: &str, started: Instant, outcome: CallOutcome)`, `record_efficiency(&self, sig: EfficiencySignal)`, `record(&self, signal: TelemetrySignal)`. Все методы возвращают `()`, ошибки sink проглатываются.
  - `OperationalSignal` получает `#[serde(default)] pub nested: bool`; `harness: Option<String>` заполняется `HarnessId::as_str`.

- [ ] **Step 1: Failing tests**
  - `harness_from_client_name_normalizes`: `Some("claude-code")`→ClaudeCode, `Some("Antigravity")`→Antigravity, `Some("weird")`→Other, `None`→Other.
  - `harness_from_env_detects_claude`: замыкание, возвращающее `true` только для `"CLAUDECODE"` → ClaudeCode; всегда `false` → Terminal.
  - `recorder_writes_operational_v2`: `JsonlTelemetrySink` во tempdir (enabled), `record_call` с `CallOutcome::Err(ErrorClass::Io)` → в JSONL одна строка, `schema_version == 2`, `signal.Operational.harness == "claude-code"`, `nested == true`, `error_class == "io"`.
  - `recorder_swallows_sink_errors`: мок-sink, чей `record` возвращает `Err` → `record_call` не паникует (Review Focus 1).
  - `reads_v1_events_without_nested`: строка JSONL v1 без `nested` читается через `read_jsonl_file`, `nested == false`.
  - `classify_error_maps_io_and_not_found`.
- [ ] **Step 2:** `rtk cargo nextest run -p ci-core -E 'test(harness_) | test(recorder_) | test(reads_v1_) | test(classify_error)'` → FAIL (не компилируется).
- [ ] **Step 3:** реализовать типы и функции по Interfaces; `Recorder::record_call` строит `OperationalSignal { tool, channel: "mcp"|"cli", harness, latency_ms, success, error_class, version: ci_core::VERSION, os: std::env::consts::OS, nested }`.
- [ ] **Step 4:** тот же запуск → PASS; полный `rtk cargo nextest run -p ci-core` → PASS.
- [ ] **Step 5:** commit `feat(telemetry): add Recorder, harness detection and schema v2`.

### Task 2: инструментирование MCP-сервера

**Files:**
- Modify: `crates/ci-mcp/src/server.rs` (поле `client_harness: Arc<RwLock<HarnessId>>`, `on_initialized`, `run_mcp_server`, хелпер `instrumented`, все 12 `#[tool]`)
- Modify: `crates/ci-mcp/src/handlers.rs` (`ResolvedProject::recorder(&self, sink) -> Recorder`)
- Test: `crates/ci-mcp/src/lib.rs` (`mod tests`, рядом с `MockTelemetrySink`)

**Interfaces:**
- Consumes: Task 1 (`Recorder`, `CallContext`, `harness_from_client_name`, `classify_error`).
- Produces:
  - `ContextInitServer::instrumented<F, Fut>(&self, tool: &'static str, project_enabled: bool, f: F) -> Result<String, rmcp::ErrorData>` где `F: FnOnce() -> Fut, Fut: Future<Output = anyhow::Result<String>>`; маппинг ошибки в `rmcp::ErrorData::internal_error` — как сейчас.
  - `ContextInitServer::call_context(&self) -> CallContext` (`channel = Mcp`, `nested = false`).
  - `run_mcp_server` создаёт `JsonlTelemetrySink::new(data_dir, true)` из `StoragePaths` глобальной конфигурации и вызывает `with_telemetry_sink`.
  - Решение «писать ли» — `project.config.telemetry.enabled` проекта, разрешённого `resolve_context` для этого вызова; при `false` `Recorder::disabled()`.

- [ ] **Step 1: Failing tests**
  - `every_tool_records_one_operational_event`: сервер с `MockTelemetrySink` и проектом с `telemetry.enabled = true`; вызвать каждый из 12 инструментов (минимальные валидные параметры; `context_submit` с неизвестным `package_id`) → ровно 12 `Operational` событий, имена совпадают с `name = "context_*"`.
  - `client_info_sets_harness`: после установки `client_harness` из `harness_from_client_name(Some("claude-code"))` событие содержит `harness == "claude-code"`; без установки — `"other"` (Review Focus 2).
  - `project_disabled_telemetry_writes_nothing`: глобальный sink есть, проект с `telemetry.enabled = false` → 0 событий (Review Focus 3).
  - `failed_tool_records_error_class`: `context_get_subsystem` с несуществующим id → `success == false`, `error_class == "not_found"`.
- [ ] **Step 2:** `rtk cargo nextest run -p ci-mcp -E 'test(every_tool_records) | test(client_info_sets) | test(project_disabled_telemetry) | test(failed_tool_records)'` → FAIL.
- [ ] **Step 3:** в `on_initialized` читать `context.peer.peer_info().map(|i| i.client_info.name.as_str())`, сохранять `HarnessId` до `refresh_roots`; обернуть тела всех `#[tool]` в `instrumented`.
- [ ] **Step 4:** тесты → PASS; `rtk cargo nextest run -p ci-mcp` → PASS.
- [ ] **Step 5:** commit `feat(mcp): record operational telemetry with client harness`.

### Task 3: сигналы эффективности

**Files:**
- Modify: `crates/ci-core/src/storage.rs` (`pub fn subsystem_tokens(index: &L0Index, ids: &[&str]) -> usize`)
- Modify: `crates/ci-mcp/src/handlers.rs` (`pack`, `get_subsystem`, `get_subsystems`, `search`, `task` принимают `tel: &TelemetryScope`)
- Modify: вызовы этих обработчиков в `server.rs` и в `src/cli/{pack,search,task}.rs`
- Test: `crates/ci-mcp/src/lib.rs`

**Interfaces:**
- Consumes: Task 1, Task 2.
- Produces: `pub struct TelemetryScope { pub recorder: Recorder, pub ctx: CallContext }` в `ci-core::telemetry`, `TelemetryScope::disabled()`, `TelemetryScope::efficiency(&self, tool: &str, source_tokens: usize, response: &str, truncated: bool, cursor_hops: usize, stale: bool, empty_search: bool)`; `response_tokens` считается внутри через `ci_mcp::pagination::count_tokens` — поэтому функция подсчёта переносится в `ci-core` (`ci_core::tokens::count_tokens`), `ci-mcp::pagination` реэкспортирует её.
- `source_tokens` по таблице спецификации §A5: `pack`/`get_subsystem` — затронутые id; `get_subsystems` — все; `search` — id из выдачи; `task` — затронутые подсистемы.
- `cursor_hops` — `page_no` из декодированного курсора (0 без курсора).

- [ ] **Step 1: Failing tests**
  - `pack_records_efficiency`: индекс с подсистемой `tokens = 5000`, `pack` → одно событие `Efficiency { tool: "context_pack", source_tokens: 5000, response_tokens > 0 }`.
  - `search_empty_sets_flag`: запрос без совпадений → `empty_search == true`, `source_tokens == 0`.
  - `cursor_hops_counted`: второй страницы `get_subsystems` с курсором → `cursor_hops == 1`.
- [ ] **Step 2:** `rtk cargo nextest run -p ci-mcp -E 'test(pack_records_efficiency) | test(search_empty_sets) | test(cursor_hops_counted)'` → FAIL.
- [ ] **Step 3:** реализовать; CLI-вызовы до Task 4 передают `TelemetryScope::disabled()`.
- [ ] **Step 4:** PASS; `rtk cargo nextest run --workspace` → PASS.
- [ ] **Step 5:** commit `feat(telemetry): record response vs source tokens per tool`.

### Task 4: инструментирование CLI и маркеры Antigravity

**Files:**
- Modify: `src/cli/mod.rs` (`run()`: обёртка диспетчера)
- Modify: `crates/ci-core/src/telemetry/harness.rs` (`ANTIGRAVITY_ENV_MARKERS`)
- Modify: `src/cli/{pack,search,task}.rs` (передать реальный `TelemetryScope`)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: Task 1, Task 3.
- Produces: `fn command_tool_name(cmd: &Commands) -> Option<&'static str>` — `init|update|check|scan|task|workspace|search|pack|config|migrate|setup|feedback`; `Mcp` и `Telemetry` → `None` (не инструментируются). `CallContext { channel: Cli, harness: harness_from_env(|k| std::env::var_os(k).is_some()), nested: harness != Terminal }`.

- [ ] **Step 1: Замерить маркеры Antigravity.** Run: `agy -p "Выполни команду env и выведи только имена переменных, начинающиеся с ANTIGRAVITY, JETSKI, AGY или GEMINI" --dangerously-skip-permissions --mode plan` в пустом временном каталоге. Записать найденные имена (только имена) в `ANTIGRAVITY_ENV_MARKERS`. Если ни одного — оставить массив пустым и добавить в README строку «Antigravity CLI определяется как terminal».
- [ ] **Step 2: Failing tests** в `tests/cli.rs` (sandbox с изолированным HOME/XDG, `telemetry.enabled = true` в глобальном конфиге sandbox):
  - `cli_records_claude_harness`: `context-init check --compact` с `CLAUDECODE=1` → в `telemetry/*.jsonl` событие `tool == "check"`, `channel == "cli"`, `harness == "claude-code"`, `nested == true`.
  - `cli_without_markers_is_terminal`: env очищен от `CLAUDECODE` → `harness == "terminal"`, `nested == false`.
  - `cli_mcp_subcommand_not_instrumented`: `context-init telemetry report` не добавляет событий.
- [ ] **Step 3:** `rtk cargo nextest run --test cli -E 'test(cli_records_claude) | test(cli_without_markers) | test(cli_mcp_subcommand)'` → FAIL.
- [ ] **Step 4:** реализовать обёртку: `started = Instant::now()`, вызвать текущий `match`, по `Result<ExitCode>` записать `CallOutcome` (`ExitCode` ≠ 0 без ошибки → `Ok`), вернуть исходный результат без изменений.
- [ ] **Step 5:** PASS; `rtk cargo nextest run --workspace` → PASS.
- [ ] **Step 6:** commit `feat(cli): record CLI telemetry with harness detection`.

### Task 5: расширенный `telemetry report`

**Files:**
- Modify: `crates/ci-core/src/telemetry.rs` (`render_telemetry_report`)
- Test: там же, снапшот-тест

**Interfaces:**
- Consumes: события v1/v2.
- Produces: в Markdown-отчёте новые разделы `## По харнессам и каналам` (таблица `харнесс | канал | инструмент | вызовы | ошибки, % | p50, мс`), `## Вложенные вызовы` (доля `nested`), `## Экономия токенов` (`Σ(source − response)` по харнессу, отрицательная разница учитывается как 0), `## Пустой поиск` (доля `empty_search` среди `context_search`/`search`). Эффективность без харнесса агрегируется под `unknown` (у `EfficiencySignal` нет харнесса; связывать по порядку событий не требуется).

- [ ] **Step 1: Failing test** `report_groups_by_harness_and_channel`: фиксированный набор — 3 операционных (claude-code/mcp/context_pack ×2 успешных, antigravity/cli/pack ×1 с ошибкой), 2 эффективности (5000→1000, 2000→3000) → отчёт содержит строку таблицы с `claude-code | mcp | context_pack | 2 | 0`, `antigravity | cli | pack | 1 | 100`, и `Сэкономлено токенов: 4000`.
- [ ] **Step 2:** `rtk cargo nextest run -p ci-core -E 'test(report_groups_by_harness)'` → FAIL.
- [ ] **Step 3:** реализовать; p50 — медиана `latency_ms` (для чётного числа — нижняя из двух средних).
- [ ] **Step 4:** PASS; существующие тесты отчёта PASS.
- [ ] **Step 5:** commit `feat(telemetry): report usage by harness, channel and token savings`.

### Task 6: аудитор транскриптов `scripts/audit-sessions.py`

**Files:**
- Create: `scripts/audit-sessions.py`, `scripts/tests/test_audit_sessions.py`, `scripts/tests/fixtures/claude/proj/{main.jsonl,main/subagents/agent-1.jsonl,main/subagents/agent-1.meta.json}`
- Modify: `mise.toml` (`[tasks.audit-sessions]` → `python3 -I scripts/audit-sessions.py`)

**Interfaces:**
- CLI: `python3 -I scripts/audit-sessions.py [--claude DIR] [--antigravity DIR] [--project SUBSTR] [--json]`; по умолчанию `~/.claude/projects` и `~/.gemini/antigravity*/conversations`.
- Вывод: строка на (проект, тип агента): `files`, `ci_mcp`, `ci_cli`, `read`, `search_shell` (`grep|rg|find|fd` в Bash), `bash`; для Antigravity — число баз и частоты имён `context_*` (поиск `rb"context-init[_:]+context_[a-z_]+"` в байтах).
- Тип агента: `main` или `subagent:<agentType>` из `.meta.json`.
- `ci_cli` — Bash-команда, содержащая `context-init (init|update|check|pack|search|task|scan|workspace)`.

- [ ] **Step 1: Failing test** (`python3 -I -m unittest scripts/tests/test_audit_sessions.py`): фикстура main с 2 вызовами `mcp__context-init__context_search` и 1 `Read`, субагент `Explore` с 1 Bash `rtk rg foo` и 1 Bash `context-init pack x --compress` → JSON-вывод `{"proj": {"main": {"ci_mcp": 2, "read": 1}, "subagent:Explore": {"ci_cli": 1, "search_shell": 1}}}`.
- [ ] **Step 2:** запуск → FAIL.
- [ ] **Step 3:** реализовать (только stdlib, ничего не пишет на диск).
- [ ] **Step 4:** PASS; ручной прогон `rtk mise run audit-sessions -- --project context-init` печатает таблицу без исключений.
- [ ] **Step 5:** commit `feat(scripts): add transcript auditor for context-init adoption`.

## Этап B — целостность конвейера

### Task 7: L0 v4, происхождение сводок и build-stamp

**Files:**
- Modify: `crates/ci-core/src/storage.rs` (`SubsystemInfo`, `L0Index`, `CURRENT_L0_SCHEMA_VERSION = 4`, `from_json_str`)
- Create: `crates/ci-core/build.rs`
- Modify: `crates/ci-core/src/lib.rs` (`pub const BUILD: &str = env!("CONTEXT_INIT_BUILD")`), `src/cli/mod.rs` (`#[command(version = ...)]` → `"{VERSION} ({BUILD})"`), `crates/ci-mcp/src/server.rs` (`get_info().server_info.version` = `"{VERSION}+{BUILD}"`)
- Test: `crates/ci-core/src/storage.rs` tests

**Interfaces:**
- Produces:
  - `pub enum SummarySource { Heuristic, Llm, Agent, Manual }` (serde `lowercase`, `Default = Heuristic`).
  - `SubsystemInfo` + `#[serde(default)] pub summary_source: SummarySource`, `#[serde(default, skip_serializing_if = "Option::is_none")] pub summary_model: Option<String>`, `#[serde(default, skip_serializing_if = "std::ops::Not::not")] pub needs_review: bool`.
  - `pub struct GeneratorStamp { pub version: String, pub build: String }`; `L0Index` + `#[serde(default, skip_serializing_if = "Option::is_none")] pub generator: Option<GeneratorStamp>`; `generate`/`update` всегда пишут `Some(GeneratorStamp { version: VERSION, build: BUILD })`.
  - `L0Index::from_json_str` принимает v3 и v4; для v3: `summary_source = Heuristic`, если `summary` начинается с одного из `GENERATED_SUMMARY_PREFIXES`, иначе `Manual`; `schema_version` в памяти = 4. Иные версии — прежняя ошибка.
  - `build.rs`: `git rev-parse --short HEAD` (+ `-dirty`, если `git status --porcelain` непуст), при ошибке `unknown`; `cargo:rustc-env=CONTEXT_INIT_BUILD=...`; `cargo:rerun-if-changed=../../.git/HEAD` и `../../.git/index`.

- [ ] **Step 1: Failing tests**
  - `v3_index_migrates_with_manual_detection`: v3 JSON с двумя подсистемами — `summary: "Модуль-файл `x.rs`"` (эвристический префикс) и `"Ручное описание"` → `Heuristic` и `Manual`, `schema_version == 4` (Review Focus 4, часть 1).
  - `v4_roundtrip_keeps_provenance`: `Agent` + `summary_model = Some("agent")` + `needs_review = true` сохраняются через save/load.
  - `v2_index_still_rejected`.
  - `build_stamp_is_set`: `!ci_core::BUILD.is_empty()`.
- [ ] **Step 2:** `rtk cargo nextest run -p ci-core -E 'test(v3_index_migrates) | test(v4_roundtrip) | test(v2_index_still) | test(build_stamp)'` → FAIL.
- [ ] **Step 3:** реализовать.
- [ ] **Step 4:** PASS; `rtk cargo nextest run --workspace` → PASS (снапшоты, сравнивающие JSON индекса, обновить, если меняется только `schema_version`/`generator`).
- [ ] **Step 5:** commit `feat(storage): L0 schema v4 with summary provenance and generator stamp`.

### Task 8: рендер из индекса и слияние по происхождению

**Files:**
- Modify: `crates/ci-core/src/analysis/generator.rs` (`update`, `render_subsystem_card_okf`, новая `apply_summaries`)
- Test: `generator.rs` tests

**Interfaces:**
- Consumes: Task 7.
- Produces:
  - `update` — для существующей подсистемы: сводка перезаписывается детектором только при `summary_source == Heuristic`; при `summary_source != Heuristic` и изменившемся множестве путей `files` → `needs_review = true`. Проверка «нетронутости» карточки — как сейчас, против рендера индекса до слияния.
  - `render_subsystem_card_okf(sub, analysis, shas, role, model)` вызывается с `model = sub.summary_model.as_deref().unwrap_or("none")` и `role` = `"agent"` для `Agent`, `"llm"` для `Llm`, иначе текущая роль.
  - `pub struct SummaryUpdate { pub id: String, pub name: Option<String>, pub summary: String, pub tags: Option<Vec<String>> }`.
  - `pub fn apply_summaries(analysis: &ProjectAnalysis, knowledge_dir: &Path, updates: &[SummaryUpdate], source: SummarySource, model: Option<&str>, agents_md: Option<StorageMode>) -> Result<usize>` — загружает индекс; отклоняет весь вызов (`bail!`, ничего не пишет), если любой id отсутствует в индексе; для каждой подсистемы определяет «нетронутость» карточки против рендера ДО слияния; сливает поля, ставит `source/model`, `needs_review = false`; перерисовывает нетронутые карточки, `L0_index.md`, `L0_index.json` и (если `agents_md`) AGENTS.md; все записи — во временные файлы того же каталога, затем `rename`; возвращает число применённых подсистем.

- [ ] **Step 1: Failing tests**
  - `apply_summaries_rerenders_l1_l0_and_agents`: `generate` во tempdir → `apply_summaries` с одной сводкой `"Новая сводка"` (`Agent`, `Some("agent")`) → строка есть в карточке, `L0_index.md`, AGENTS.md; frontmatter карточки `generated_by.role == "agent"`, `model == "agent"`.
  - `update_after_apply_keeps_agent_summary`: затем `update` с тем же анализом → сводка и карточка не изменились.
  - `update_marks_needs_review_on_file_change`: добавить файл в подсистему → `needs_review == true`, сводка агента сохранена.
  - `manual_summary_survives_update`: v3-индекс с ручной сводкой (из Task 7) → `update` → сводка и карточка не перезаписаны (Review Focus 4, часть 2).
  - `apply_summaries_unknown_id_changes_nothing`: апдейты `[valid, "nope"]` → `Err`, байты `L0_index.json` и карточки идентичны исходным.
- [ ] **Step 2:** `rtk cargo nextest run -p ci-core -E 'test(apply_summaries_) | test(update_after_apply) | test(update_marks_needs_review) | test(manual_summary_survives)'` → FAIL.
- [ ] **Step 3:** реализовать.
- [ ] **Step 4:** PASS; `rtk cargo nextest run --workspace` → PASS.
- [ ] **Step 5:** commit `fix(generator): render cards from index summaries and respect provenance`.

### Task 9: `context_submit` и LLM-обогащение через `apply_summaries`

**Files:**
- Modify: `crates/ci-mcp/src/handlers.rs` (`persist_submission`, `submit`)
- Modify: `crates/ci-mcp/src/pipeline.rs` (после `generate`/`update` применять результат обогащения через `apply_summaries`)
- Modify: `crates/ci-llm/src/enrich.rs` (`EnrichOutcome::Enriched` несёт `Vec<SummaryUpdate>` и `model: String` вместо мутации `subsystems`)
- Test: `crates/ci-mcp/src/lib.rs`

**Interfaces:**
- Consumes: Task 8 (`apply_summaries`, `SummaryUpdate`).
- Produces:
  - `EnrichOutcome::Enriched { updates: Vec<SummaryUpdate>, model: String }`; `enrich_subsystems` больше не мутирует сводки (сигнатура: `subsystems: &[SubsystemInfo]`).
  - `pipeline::run_init/run_update`: после записи базы — `apply_summaries(.., SummarySource::Llm, Some(&model), ..)` при `Enriched`.
  - `persist_submission(project, pkg, payload)`: принимает только форму `{subsystems:[...]}`; детерминированный повторный анализ с `deep` из пакета (поле `AgentWorkPackage.deep: bool`, проставляется при создании пакета) и `apply_summaries(.., SummarySource::Agent, Some(<alias пакета>), agents_md)`; `agents_md` — `project.config.touch_agents_md`-логика как в `run_update`. Ветка одиночной карточки с `body` удаляется.
  - Ошибка `apply_summaries` или анализа → `SubmitResult { valid: false, errors: [..] }`.

- [ ] **Step 1: Failing tests**
  - `submit_updates_l1_card_and_l0_md`: `context_init` (агентская модель) → `context_submit` со сводкой для одного id → новая сводка в L1-карточке и `L0_index.md`; `context_check` → без дрейфа.
  - `submit_with_unknown_id_is_rejected_atomically`: `[valid, "ghost"]` → `valid == false`, `L0_index.json` побайтно не изменён (Review Focus 5).
  - `submit_rejects_single_card_body_form`: `{id, summary, body}` → `valid == false`.
  - `llm_enrichment_sets_llm_source`: мок-провайдер `command` возвращает сводку → в индексе `summary_source == "llm"`, `summary_model == <alias>`.
- [ ] **Step 2:** `rtk cargo nextest run -p ci-mcp -E 'test(submit_) | test(llm_enrichment_sets)'` → FAIL.
- [ ] **Step 3:** реализовать.
- [ ] **Step 4:** PASS; `rtk cargo nextest run --workspace` → PASS.
- [ ] **Step 5:** commit `fix(mcp): apply agent submissions to the whole knowledge base`.

### Task 10: единая схема `update` и обогащение только изменённых

**Files:**
- Modify: `crates/ci-llm/src/schema.rs` (`default_role_json_schema(Update)` = схема `Init`)
- Modify: `crates/ci-llm/src/prompts.rs` (инструкция роли `update`: формат `{"subsystems": [...]}`, «верни только перечисленные подсистемы»)
- Modify: `crates/ci-mcp/src/pipeline.rs` (`analyze` для `Update` фильтрует подсистемы)
- Test: `crates/ci-llm/src/lib.rs`, `crates/ci-mcp/src/lib.rs`

**Interfaces:**
- Consumes: Task 7, Task 9.
- Produces: `pub fn subsystems_needing_enrichment(existing: &L0Index, fresh: &[SubsystemInfo]) -> Vec<String>` в `pipeline.rs` — id новых подсистем, подсистем с изменившимся множеством путей `files`, с `needs_review`, и с `summary_source == Heuristic`. `analyze(Update)` передаёт в `enrich_subsystems` только их; пустой список → `EnrichOutcome::Skipped`, пакет не создаётся; MCP-ответ `context_update` содержит строку `Обогащение не требуется: изменений нет.`

- [ ] **Step 1: Failing tests**
  - `update_schema_matches_init`: `default_role_json_schema(Update) == default_role_json_schema(Init)`.
  - `update_package_lists_only_changed`: база после `apply_summaries` для всех id (все `Agent`) → изменить один файл → `context_update` с агентской моделью → в facts пакета ровно один id.
  - `update_without_changes_creates_no_package`: повторный `context_update` → ответ содержит `Обогащение не требуется`, `work_packages` пуст.
- [ ] **Step 2:** `rtk cargo nextest run --workspace -E 'test(update_schema_matches) | test(update_package_lists) | test(update_without_changes)'` → FAIL.
- [ ] **Step 3:** реализовать.
- [ ] **Step 4:** PASS; полный прогон PASS.
- [ ] **Step 5:** commit `fix(llm): unify update schema and enrich only changed subsystems`.

### Task 11: дрейф `format`, футер рассинхрона, `mise run install`

**Files:**
- Modify: `crates/ci-core/src/storage.rs` (`DriftReport` + `pub format_issues: Vec<String>`), `crates/ci-core/src/analysis/generator.rs` (`audit`)
- Modify: `crates/ci-mcp/src/pipeline.rs` (`FullDriftReport.format_issues`, `has_drift` учитывает их), `src/cli/check.rs` (вывод и `--exit-code`)
- Modify: `crates/ci-mcp/src/server.rs` (`exe_mtime_at_start: Option<SystemTime>`, футер)
- Modify: `mise.toml` (`[tasks.install]`), `README.md` (раздел «Установка из исходников»)
- Test: `generator.rs` tests, `crates/ci-mcp/src/lib.rs`, `tests/cli.rs`

**Interfaces:**
- Consumes: Task 7 (`GeneratorStamp`).
- Produces:
  - `audit` добавляет в `format_issues`: `"<id>: missing OKF frontmatter"` для карточки, которую `OkfCard::parse` не принимает; `"generator <v> is newer than running <VERSION>"`, если `index.generator.version` > `ci_core::VERSION` по semver (сравнение через `semver` уже в workspace либо покомпонентно `u64`).
  - `ContextInitServer::stale_binary_footer(&self) -> Option<&'static str>` — `Some("⚠ context-init обновлён на диске — перезапустите MCP-сервер")`, если mtime `std::env::current_exe()` сейчас ≠ запомненному при старте; `instrumented` дописывает футер отдельной строкой к успешному ответу.
  - `mise.toml`:
    ```toml
    [tasks.install]
    description = "Build release binary and install it to ~/.local/bin atomically"
    run = "cargo build --release --bin context-init && install -m755 target/release/context-init ~/.local/bin/.context-init.new && mv ~/.local/bin/.context-init.new ~/.local/bin/context-init"
    ```

- [ ] **Step 1: Failing tests**
  - `audit_flags_card_without_frontmatter`: убрать frontmatter у одной карточки → `format_issues` содержит её id, `has_drift`.
  - `audit_flags_newer_generator`: `generator.version = "99.0.0"` → issue.
  - `cli_check_exit_code_on_format_drift` (`tests/cli.rs`): `check --exit-code` → код 1.
  - `stale_binary_footer_appears_when_mtime_changes`: сервер с подменённым `exe_mtime_at_start = Some(UNIX_EPOCH)` → ответ `context_check` оканчивается футером; с совпадающим mtime — без футера.
- [ ] **Step 2:** `rtk cargo nextest run --workspace -E 'test(audit_flags_) | test(cli_check_exit_code_on_format) | test(stale_binary_footer)'` → FAIL.
- [ ] **Step 3:** реализовать; README — 3 строки: `rtk mise run install` после изменений, затем перезапуск MCP-сервера в харнессе.
- [ ] **Step 4:** PASS; `rtk mise run install` устанавливает бинарь, `context-init --version` печатает `0.3.0 (<sha>)`.
- [ ] **Step 5:** commit `feat(check): detect format drift and stale MCP binary; add mise install`.

## Этап D — качество данных

### Task 12: тестовые файлы без символов

**Files:**
- Modify: `crates/ci-core/src/analysis/detector.rs` (`extract_symbols_from_files`, `extract_rust_symbols`)
- Modify: `crates/ci-core/src/analysis/subsystems.rs` (тег и сводка для тестовых подсистем)
- Test: `crates/ci-core/src/analysis/detector/tests.rs`

**Interfaces:**
- Produces:
  - `pub fn is_test_path(rel: &str) -> bool` — `true` для пути с сегментом `tests/`, имени `tests.rs`, суффиксов `_test.rs`, `_tests.rs`, `_test.py`, `test_*.py`.
  - `extract_symbols_from_files` пропускает файлы с `is_test_path`; `extract_rust_symbols` пропускает элементы внутри `#[cfg(test)] mod`.
  - Подсистема, у которой все файлы `is_test_path`: тег `tests` добавлен, эвристическая сводка `Тесты для `<parent id>`` (при отсутствии `parent` — `Тесты проекта`); префикс `Тесты для ` добавляется в `GENERATED_SUMMARY_PREFIXES`.

- [ ] **Step 1: Failing tests**
  - `test_files_contribute_no_symbols`: временный проект с `src/a.rs` (`pub fn real()`) и `src/a/tests.rs` (`pub struct Engine;`) → `key_symbols` не содержит `Engine`, содержит `real`.
  - `cfg_test_module_symbols_skipped`: `pub fn real(){}` + `#[cfg(test)] mod t { pub struct Fake; }` → только `real`.
  - `test_only_subsystem_tagged`: подсистема только из `tests.rs` → тег `tests`, сводка начинается с `Тесты для `.
- [ ] **Step 2:** `rtk cargo nextest run -p ci-core -E 'test(test_files_contribute) | test(cfg_test_module) | test(test_only_subsystem)'` → FAIL.
- [ ] **Step 3:** реализовать.
- [ ] **Step 4:** PASS; полный прогон PASS.
- [ ] **Step 5:** commit `fix(detector): keep test fixtures out of subsystem symbols`.

### Task 13: понижение тестовых подсистем в поиске

**Files:**
- Modify: `crates/ci-core/src/search.rs` (`Entry.tags`, пересортировка)
- Test: `search.rs` tests

**Interfaces:**
- Consumes: тег `tests` из Task 12.
- Produces: в `Bm25Retriever::search` выборка `TopDocs::with_limit(top_k * 2)`, скор подсистем с тегом `tests` умножается на `0.5`, затем сортировка по убыванию и усечение до `top_k`.

- [ ] **Step 1: Failing test** `tests_subsystem_ranks_below_its_parent`: две подсистемы — `detector` (summary «детектор проекта») и `detector_tests` (тег `tests`, summary «тесты детектора проекта детектор») → по запросу `детектор` первым идёт `detector`.
- [ ] **Step 2:** `rtk cargo nextest run -p ci-core -E 'test(tests_subsystem_ranks)'` → FAIL.
- [ ] **Step 3:** реализовать.
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** commit `feat(search): demote test-only subsystems`.

## Этап C — использование агентами

### Task 14: маршрутизирующие описания MCP-инструментов

**Files:**
- Modify: `crates/ci-mcp/src/server.rs` (`description` всех 12 `#[tool]`)
- Test: `crates/ci-mcp/src/lib.rs`

**Interfaces:**
- Produces: описание каждого инструмента = «что делает. Когда: <триггер>. <выгода/ориентир токенов>.», ≤ 300 символов, на английском (как текущие). Обязательные триггеры:
  - `context_search`: «Start here when you do not know which module handles a concept; returns ranked subsystems with signatures (~1-2k tokens) instead of reading files.»
  - `context_get_subsystems`: «Use at the start of unfamiliar work to see the whole map.»
  - `context_get_subsystem`: «Use before opening files of one subsystem.»
  - `context_pack`: «Use instead of reading 3+ files of one subsystem; bodies elided.»
  - `context_task`: «Use with a diff/branch to see which subsystems a change touches.»
  - `context_check`: «Run before finishing a task that changed structure.»
  - для `context_init/update/submit/scan/workspace/feedback` — текущий смысл + триггер одной фразой.

- [ ] **Step 1: Failing test** `tool_descriptions_have_triggers_and_fit`: из `ContextInitServer::tool_router().list_all()` у каждого инструмента `description.len() <= 300`, у `context_search/get_subsystems/get_subsystem/pack/task/check` описание содержит `Use ` или `Start here` или `Run before`.
- [ ] **Step 2:** `rtk cargo nextest run -p ci-mcp -E 'test(tool_descriptions_have)'` → FAIL.
- [ ] **Step 3:** обновить описания.
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** commit `feat(mcp): routing hints in tool descriptions`.

### Task 15: правило `setup` для основной сессии и субагентов

**Files:**
- Modify: `src/cli/setup/rule_template.md`, `src/cli/setup/rules.rs` (лимит строк в тесте 15 → 25; подстановка rtk)
- Test: `src/cli/setup/rules.rs` tests, `tests/cli.rs`

**Interfaces:**
- Produces: шаблон — два раздела: `### Main session (MCP)` (текущие 4 шага) и `### Subagents and scripts (CLI)`:
  ```
  Before reading code broadly, run `{rtk}context-init search "<question>" --compact`,
  then `{rtk}context-init pack <subsystem_id> --compress` for the best hit.
  For a known symbol or string, `rg` is faster — use it.
  ```
  `{rtk}` заменяется на `rtk ` при активной интеграции rtk (`integrations.resolve_rtk_active()`), иначе пустая строка; `pub fn render_rule(rtk: bool) -> String`.

- [ ] **Step 1: Failing tests**
  - `rule_template_is_at_most_25_lines` (переименованный старый тест).
  - `render_rule_prefixes_rtk`: `render_rule(true)` содержит `rtk context-init search`, `render_rule(false)` — `context-init search` без `rtk `.
  - `setup_writes_subagent_section` (`tests/cli.rs`): `context-init setup claude-code` в sandbox → целевой файл правил содержит `Subagents and scripts`.
- [ ] **Step 2:** `rtk cargo nextest run --workspace -E 'test(rule_template_is_at_most_25) | test(render_rule_prefixes) | test(setup_writes_subagent)'` → FAIL.
- [ ] **Step 3:** реализовать.
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** commit `feat(setup): add subagent/CLI section to the agent rule`.

### Task 16: пробы использования `scripts/probe-adoption.sh`

**Files:**
- Create: `scripts/probe-adoption.sh`, `scripts/probes/01-telemetry-wiring.md`, `02-card-rendering.md`, `03-agent-handoff-flow.md` (обзорные), `04-default-tokens.md`, `05-search-boost.md` (точечные)
- Modify: `mise.toml` (`[tasks.probe-adoption]`)

**Interfaces:**
- Consumes: Task 6 (`audit-sessions.py --json`), Task 15.
- Probe-файл: `## Question` (текст для агента, без упоминания context-init), `## Kind` (`overview|pointed`), `## Expected` (чек-лист из 2–4 фактов с `file:line`-якорями).
- `probe-adoption.sh [--runs N] [--baseline]`: для каждой пробы и каждого прогона — `claude -p "<Question> Делегируй исследование субагенту." --output-format json` и `agy -p "<Question>" --mode plan --output-format stream-json`, рабочий каталог — корень репозитория; `--baseline` запускает только `claude` с `--strict-mcp-config --mcp-config <файл с "mcpServers": {}>` (у `agy` нет флага подмены MCP-конфига; его базой служат прогоны, где context-init не вызывался, — это фиксируется в отчёте как ограничение); результаты в `${TMPDIR:-/tmp}/context-init-probes/<timestamp>/`; в конце печатает таблицу: проба, харнесс, вызовы context-init (из `audit-sessions.py --json` по session id / conversation id), токены (из JSON-итога харнесса), ответы для ручной сверки с `## Expected`.
- Критерий S5 проверяется вручную по таблице (медиана 3 прогонов).

- [ ] **Step 1:** написать 5 probe-файлов с эталонами по текущему коду (эталоны сверить чтением соответствующих мест).
- [ ] **Step 2:** `bash -n scripts/probe-adoption.sh` → без ошибок; `shellcheck scripts/probe-adoption.sh` (если установлен) → без ошибок.
- [ ] **Step 3:** пробный прогон `rtk mise run probe-adoption -- --runs 1` на одной пробе → таблица из 2 строк (claude, agy) без исключений.
- [ ] **Step 4:** commit `feat(scripts): add adoption probes for claude and antigravity`.

### Task 17: финальная проверка и обновление базы

**Files:**
- Modify: `.context-init/knowledge/**`, `AGENTS.md`, `CHANGELOG.md` (раздел Unreleased)

- [ ] **Step 1:** `rtk cargo clippy --workspace --all-targets -- -D warnings` и `rtk cargo nextest run --workspace` → PASS.
- [ ] **Step 2:** `rtk mise run install`; переподключить MCP-сервер в харнессе.
- [ ] **Step 3:** `rtk context-init update --deep .` и `rtk context-init check --exit-code --compact` → `OK`.
- [ ] **Step 4:** включить телеметрию, выполнить по одному вызову `search`/`pack` через MCP и CLI → `rtk context-init telemetry report` показывает строки `claude-code | mcp` и `claude-code | cli`.
- [ ] **Step 5:** `rtk mise run probe-adoption -- --runs 3` и `--baseline`; записать таблицу S5 в `reports/adoption_probe_2026-10.md`.
- [ ] **Step 6:** commit `chore(kb): refresh knowledge base after adoption and integrity work` (база, CHANGELOG, отчёт проб).
