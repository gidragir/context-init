# context-init: наблюдаемость, целостность конвейера и использование агентами

**Дата:** 2026-10-29
**Статус:** дизайн согласован, ожидает ревью спецификации
**Основание:** аудит сессии 2026-10-29 (полная регенерация базы + проверка использования в Claude Code, субагентах Claude и Antigravity)

## 1. Цель и критерии успеха

context-init должен (1) реально применяться агентами там, где он выгоднее чтения кода, (2) позволять измерить это применение и пользу, (3) поддерживать базу знаний согласованной без ручных обходов.

Критерии успеха всей спецификации:

| # | Критерий | Как проверяется |
|---|---|---|
| S1 | Каждый вызов MCP/CLI при `[telemetry].enabled = true` даёт операционную запись с `harness` и `channel` | интеграционные тесты A, `telemetry report` после реальной сессии |
| S2 | Один `context_submit` даёт согласованную базу: индекс, L1, `L0_index.md`, AGENTS.md | тесты B |
| S3 | Устаревший бинарник или карточка без OKF-frontmatter обнаруживаются `check` или футером ответа | тесты B5 |
| S4 | Тестовые фикстуры не попадают в `key_symbols` | снапшот D |
| S5 | На обзорных вопросах проб каждый харнесс делает ≥1 вызов context-init, токенов не больше базового прогона, корректность не хуже | `scripts/probe-adoption.sh` (C3) |

## 2. Исходные факты (аудит)

1. `OperationalSignal`/`EfficiencySignal` в продакшене не создаются; `run_mcp_server` (`crates/ci-mcp/src/server.rs:549`) не вызывает `with_telemetry_sink`; `clientInfo` не сохраняется. При включённой телеметрии пишется только `feedback`.
2. `context_submit` с массивом `subsystems` обновляет только `L0_index.json`; L1-карточки остаются с эвристическим текстом; `L0_index.md`/AGENTS.md подтягиваются лишь следующим `update`.
3. Пакет роли `update` перечисляет все подсистемы, а схема ответа — одна карточка `{id, summary, body}`; `body` целиком перезаписывает OKF-карточку, теряя frontmatter.
4. Установленный `~/.local/bin/context-init` и запущенный MCP-процесс отставали от `main`: карточки писались без OKF-frontmatter, `check` сообщал OK, поиск по нескольким словам давал 0 результатов (в `main` BM25 работает). Lefthook больше не устанавливает бинарь.
5. Символы тестовых фикстур (`Engine`, `MyService`) попадают в `key_symbols` подсистемы `crates_ci-core_analysis_detector_tests`.
6. Использование: в транскриптах Claude Code context-init MCP до аудита не вызывался; субагент Claude и `agy -p` на нейтральной пробе выбрали `rtk rg`/`read_file`. В Antigravity IDE context-init упоминается в ~208 из 579 бесед.

## 3. Выбранный подход

Встроенная, не зависящая от харнесса телеметрия — ядро продукта. Офлайн-аудитор транскриптов и пробы — инструменты разработчика в `scripts/`, не часть продукта: они дают разбивку по субагентам и контрфактическую метрику, но привязаны к форматам харнессов.

Отклонено: корреляция сессий через `_meta` MCP — харнессы её не передают.

Порядок этапов: **A → B → D → C**. C измеряется только после A; D мал и независим.

## 4. Этап A — наблюдаемость

### A1. Фасад `ci_core::telemetry::Recorder`

```rust
pub struct CallContext { pub harness: HarnessId, pub channel: Channel, pub nested: bool }
pub enum Channel { Mcp, Cli }
impl Recorder {
    pub fn new(sink: Option<Arc<dyn TelemetrySink>>) -> Self;
    pub fn record_call(&self, ctx: &CallContext, tool: &str, started: Instant, outcome: CallOutcome);
    pub fn record_efficiency(&self, ctx: &CallContext, sig: EfficiencySignal);
}
pub enum CallOutcome { Ok, Err(ErrorClass) }
pub enum ErrorClass { NotFound, InvalidParams, Io, Drift, Internal }
```

Ошибки записи проглатываются: телеметрия никогда не ломает вызов. `RoleRouter` и `decorators` в `ci-llm` пишут через `Recorder`, прямые `sink.record` убираются.

`OperationalSignal` получает поле `nested: bool` (версия схемы JSONL → 2; чтение v1 сохраняется, отсутствующее поле = `false`).

### A2. Определение харнесса `HarnessId`

Значения: `claude-code`, `antigravity`, `cursor`, `codex`, `gemini-cli`, `terminal`, `other`. Сырые имена и значения переменных не пишутся.

- **MCP:** `ServerHandler::initialize` читает `clientInfo.name`, нормализует по таблице подстрок и сохраняет в `ContextInitServer`. Неизвестное → `other`.
- **CLI:** по наличию переменных окружения: `CLAUDECODE` → `claude-code`; маркеры Antigravity/`agy` фиксируются при реализации замером `env` внутри `agy -p` и записываются в ту же таблицу; иначе `terminal`. `nested = harness != terminal`.

### A3. MCP-сервер

`run_mcp_server` создаёт `JsonlTelemetrySink` (`data_dir` из глобальной конфигурации; `enabled` проверяется при каждой записи по конфигурации разрешённого проекта). Каждый `#[tool]` вызывает обработчик через хелпер:

```rust
async fn instrumented<F>(&self, tool: &'static str, f: F) -> Result<String, rmcp::ErrorData>
```

Он замеряет латентность, классифицирует ошибку и пишет операционную запись. Эффективность пишет обработчик, у которого есть данные (A5).

### A4. CLI

`src/cli/mod.rs::run()` оборачивает диспетчер команд: имя подкоманды clap → `tool`, канал `cli`. Подкоманды `mcp` и `telemetry` не инструментируются (`mcp` делегирует серверу, `telemetry` — чтение отчёта).

### A5. Эффективность

| Инструмент | `source_tokens` | `response_tokens` |
|---|---|---|
| `pack` | Σ `SubsystemInfo.tokens` упакованных подсистем | `count_tokens(output)` |
| `get_subsystem` | `tokens` подсистемы | то же |
| `get_subsystems` | Σ `tokens` всех подсистем | то же |
| `search` | Σ `tokens` подсистем в выдаче | то же |
| `task` | Σ `tokens` затронутых подсистем | то же |

`truncated`, `cursor_hops`, `stale`, `empty_search` берутся из существующей пагинации и проверки свежести.

### A6. Отчёт

`context-init telemetry report` дополняется: таблица «харнесс × канал × инструмент» (вызовы, доля ошибок, p50 латентности), `nested`-доля, итог «сэкономлено токенов = Σ(source − response)» по харнессам, доля `empty_search`.

### A7. Аудитор `scripts/audit-sessions.py`

Только stdlib, запуск `python3 -I scripts/audit-sessions.py [--claude DIR] [--antigravity DIR]`. Для Claude: основные транскрипты и `*/subagents/*.jsonl` с `agentType` из `.meta.json`. Для Antigravity: базы бесед `~/.gemini/antigravity*/conversations/*.db` (поиск имён инструментов). Вывод по агенту: вызовы context-init (MCP/CLI) против `Read`/`rg`/`grep`. Ничего не пишет.

### A8. Тесты

- мок-sink в `ci-mcp`: каждый из инструментов даёт ровно одну операционную запись с `harness` из поддельного `clientInfo`;
- CLI-тест с `CLAUDECODE=1` → `harness=claude-code`, `nested=true`; без переменных → `terminal`;
- `enabled=false` → файл не создаётся;
- чтение JSONL v1 после перехода на v2;
- снапшот отчёта на фиксированных событиях.

## 5. Этап B — целостность конвейера

### B1. Происхождение сводок

`SubsystemInfo` (L0 schema v4) получает:

```rust
pub summary_source: SummarySource,      // heuristic | llm | agent | manual
pub summary_model: Option<String>,      // alias модели
pub needs_review: bool,                 // набор файлов изменился после не-эвристической сводки
```

Слияние в `update`: сводка с `summary_source ≠ heuristic` детектором не перезаписывается; если набор файлов подсистемы изменился — `needs_review = true`. Миграция v3 → v4: сводка, начинающаяся с `GENERATED_SUMMARY_PREFIXES`, → `heuristic`, иначе → `manual` (ручную правку нельзя затирать); `needs_review = false`.

Причина дефекта №2 (уточнено при планировании): `generator::update` считает карточку ручной, если она не совпадает с рендером из *текущего* индекса; после `submit` индекс уже содержит новую сводку, карточка на диске — старую, и карточка замораживается навсегда. Поэтому применение сводок (`apply_summaries`) проверяет «нетронутость» карточки против рендера *до* слияния.

### B2. Единая точка рендера

L1-карточка, `L0_index.md` и AGENTS.md рендерятся только из объединённого `SubsystemInfo` индекса. `render_subsystem_card` берёт сводку из индекса; `generated_by.role/model` во frontmatter отражают `summary_source`/`summary_model` (`heuristic` → `model: none`).

### B3. `context_submit` доводит работу до конца

`persist_submission`:
1. валидирует все id (как сейчас) — до любой записи;
2. сливает `name/summary/tags` в индекс с `summary_source = agent`, `summary_model = <alias пакета>`, `needs_review = false`; неизвестные id отклоняются (сейчас — добавляются);
3. выполняет детерминированный повторный анализ и общий шаг рендера из `ci_mcp::pipeline` (выделяется функция `render_knowledge(project, analysis, index)`).

Ветка «одиночная карточка с `body`» удаляется. Запись атомарна (временный файл + `rename` для индекса и каждой карточки). Сбой повторного анализа → индекс не меняется, ответ `valid:false` с причиной.

### B4. Единая схема ролей

`default_role_json_schema(Update)` = схема `Init`: `{subsystems:[{id,name,summary,tags}]}`. Пакет `update` содержит только подсистемы, у которых изменились файлы, или с `needs_review = true`; если таких нет — пакет не создаётся, ответ сообщает «обогащение не требуется».

### B5. Защита от рассинхрона версий

- `build.rs` в корневом крейте: `CONTEXT_INIT_BUILD = git rev-parse --short HEAD` + `-dirty` при незакоммиченных изменениях (вне Git — `unknown`). Виден в `--version`, `serverInfo.version` MCP и `L0_index.generator = {version, build}`.
- `check`: новый вид дрейфа `format` — L1-карточка без валидного OKF-frontmatter, либо `L0_index.generator.version` новее запущенного бинаря. Учитывается в `--exit-code`.
- MCP-сервер при старте запоминает mtime `std::env::current_exe()`; если на диске он изменился, каждый ответ получает в футере строку `⚠ context-init обновлён на диске — перезапустите MCP-сервер`.
- `mise.toml`: задача `install` — `cargo build --release --bin context-init` и атомарная установка в `~/.local/bin/context-init` (копия во временный файл + `mv`). README документирует `mise run install` после изменений.

### B6. Тесты

- `submit` → новая сводка есть в L1, `L0_index.md`, AGENTS.md (при `touch_agents_md`);
- детерминированный `update` после `submit` сохраняет сводку агента; изменение файлов подсистемы → `needs_review`;
- пакет `update` содержит только затронутые id; без изменений пакета нет;
- `submit` с неизвестным id → `valid:false`, база не изменена;
- карточка без frontmatter → `check` сообщает `format`;
- миграция v3 → v4 на снапшоте текущей базы репозитория;
- футер при подменённом mtime исполняемого файла.

## 6. Этап D — качество данных детектора

- **D1.** Файлы `tests/**`, `tests.rs`, `*_test.rs`, `*_tests.rs` и модули `#[cfg(test)]` не дают `key_symbols`. Подсистема, целиком состоящая из таких файлов, получает тег `tests` и эвристическую сводку «Тесты для `<родительская подсистема>`».
- **D2.** BM25-поиск применяет к документам с тегом `tests` множитель 0.5 к скору (не исключает).
- **Тесты:** снапшот — у `crates_ci-core_analysis_detector_tests` нет `Engine`/`MyService`; по запросу `detector` подсистема детектора ранжируется выше своих тестов.

## 7. Этап C — использование агентами

Принцип: маршрутизация, не принуждение. Точечный поиск символа остаётся за `rg`; context-init — для ориентации (незнакомая область, вопрос «где/что», >3 файлов на чтение).

- **C1. Описания MCP-инструментов** (`#[tool(description)]`): что делает, когда применять, чем выгоднее альтернативы, ориентир по токенам. Длина ≤ 300 символов на инструмент.
- **C2. Правило `setup`** (`src/cli/setup/rule_template.md`) — два раздела: основная сессия (MCP-маршрут) и субагенты/CLI (`context-init search "<вопрос>" --compact` → `pack <id> --compress`). `setup` пишет правило туда, где харнесс его читает (пути сверяются в `harness.rs`); rtk-префикс по `integrations.rtk`.
- **C3. Пробы** `scripts/probe-adoption.sh`: 5 вопросов к этому репозиторию с эталонными ответами в `scripts/probes/*.md` (3 обзорных, 2 точечных); прогон `claude -p` (с поручением субагенту) и `agy -p --mode plan` в режиме только чтения, базовый прогон — без MCP context-init. Метрики через A7: вызовы context-init, токены, совпадение с эталоном (ручная оценка по чек-листу эталона).
- Критерий — S5.

## 8. Вне скоупа (правила хоста, репозиторий ai-dotfiles)

Найдено аудитом, к context-init не относится:

1. `~/.config/git/ignore` не игнорирует `mise.local.toml`, `*.db`, `*.sqlite`, `*.token` (покрыты только `.env`, `*.key`).
2. `rtk hook claude` переписывает `grep`/`find` в `rtk grep`/`rtk find`, но не блокирует; нативные Grep/Glob в Claude Desktop недоступны — формулировку правила 1 в глобальном CLAUDE.md нужно согласовать с реальностью.
3. `rtk oco -y` с `OCO_PROMPT_MODULE=conventional-commit` на `qwen2.5:14b` выдал не-конвенциональное русское сообщение — нужна валидация (commitlint в lefthook `commit-msg`) или другая модель.
4. Hindsight `recall`/`retain` не вызывались ни в одной сессии проекта mimir.

## 9. Риски

| Риск | Смягчение |
|---|---|
| Маркеры окружения Antigravity неизвестны или меняются | таблица в одном месте, fallback `other`/`terminal`, тест на таблицу |
| L0 v4 ломает старые базы | миграция при загрузке, чтение v3 сохраняется |
| Повторный анализ в `submit` медленный на больших репозиториях | анализ без `deep`, если исходный пакет был без `deep`; замер в регрессионном бенчмарке |
| Пробы C3 недетерминированы | 3 прогона на вопрос, медиана; критерий — тренд, не единичный прогон |
