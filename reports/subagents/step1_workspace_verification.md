# Отчет о независимой верификации Шага 1 вехи v0.3 «Фундамент» (Cargo Workspace & Profiles)

**Дата аудита:** 2026-10-19  
**Аудитор:** DeepInvestigator (Independent Review & Verification)  
**Итоговый статус:** **PASS (все 6 категорий основного чек-листа пройдены; выявлены 3 архитектурных уточнения сверх базовой проверки)**

---

## 1. Сводная таблица верификации

| # | Пункт чек-листа | Нормативный документ | Статус | Доказательство |
| :--- | :--- | :--- | :--- | :--- |
| **1.1** | Секция `[workspace]` (`resolver = "2"`, `members = ["crates/ci-core", "crates/ci-llm", "crates/ci-mcp", "."]`) | [milestone_decisions.md](file:///home/gidragir/projects/context-init/milestone_decisions.md#L84) (D24) | **PASS** | [Cargo.toml:1-8](file:///home/gidragir/projects/context-init/Cargo.toml#L1-L8) |
| **1.2** | Профили `dev`, `dev.package."*"`, `test`, `release`, `dist` | [milestone_decisions.md](file:///home/gidragir/projects/context-init/milestone_decisions.md#L85) (D25), [orchestration_architecture.md](file:///home/gidragir/.gemini/antigravity/brain/33f3fc15-fa35-49db-85ed-27e725f95875/orchestration_architecture.md#L7-L35) | **PASS** | [Cargo.toml:62-84](file:///home/gidragir/projects/context-init/Cargo.toml#L62-L84) + `rtk cargo check --workspace --release` и `--profile dist` (exit 0) |
| **2.1** | Крейт `crates/ci-core` (`Cargo.toml`, `src/lib.rs`) | D24 | **PASS** | [Cargo.toml:1-16](file:///home/gidragir/projects/context-init/crates/ci-core/Cargo.toml#L1-L16), [lib.rs:1-21](file:///home/gidragir/projects/context-init/crates/ci-core/src/lib.rs#L1-L21) |
| **2.2** | Крейт `crates/ci-llm` (`Cargo.toml`, `src/lib.rs`) | D24 | **PASS** | [Cargo.toml:1-14](file:///home/gidragir/projects/context-init/crates/ci-llm/Cargo.toml#L1-L14), [lib.rs:1-22](file:///home/gidragir/projects/context-init/crates/ci-llm/src/lib.rs#L1-L22) |
| **2.3** | Крейт `crates/ci-mcp` (`Cargo.toml`, `src/lib.rs`) | D24 | **PASS** | [Cargo.toml:1-18](file:///home/gidragir/projects/context-init/crates/ci-mcp/Cargo.toml#L1-L18), [lib.rs:1-28](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/lib.rs#L1-L28) |
| **3.1** | Единый источник версии `#[command(..., version, ...)]` без литерала | [milestone_decisions.md](file:///home/gidragir/projects/context-init/milestone_decisions.md#L87) (D27) | **PASS** | [src/main.rs:8-15](file:///home/gidragir/projects/context-init/src/main.rs#L8-L15), тест [src/main.rs:1002-1011](file:///home/gidragir/projects/context-init/src/main.rs#L1002-L1011) |
| **3.2** | Устранение двойной компиляции модулей в `src/main.rs` | [milestone_decisions.md](file:///home/gidragir/projects/context-init/milestone_decisions.md#L7) (разд. 0) | **PASS** | [src/main.rs:4](file:///home/gidragir/projects/context-init/src/main.rs#L4), [src/lib.rs:1-6](file:///home/gidragir/projects/context-init/src/lib.rs#L1-L6) |
| **4.1** | Отсутствие упоминаний «1С» / «1C» в `Cargo.toml`, `crates/`, `src/` | [milestone_decisions.md](file:///home/gidragir/projects/context-init/milestone_decisions.md#L117) (разд. 3) | **PASS** | `rtk rg -i '1[cс]' Cargo.toml crates/ src/` → exit code 1 (0 совпадений) |
| **5.1** | `rtk cargo check --workspace` | Объективная проверка | **PASS** | Код возврата `0` (`0.12s`) |
| **5.2** | `rtk cargo check -p ci-core -p ci-llm -p ci-mcp -p context-init` | Объективная проверка | **PASS** | Код возврата `0` (`0.05s`) |
| **5.3** | `rtk cargo clippy --workspace --all-targets -- -D warnings` | Объективная проверка (D26) | **PASS** | Код возврата `0` (`No issues found`) |
| **5.4** | `rtk cargo nextest run --workspace` | Объективная проверка (D26) | **PASS** | Код возврата `0` (`39 passed, 6 binaries, 0.177s`) |
| **5.5** | `rtk cargo check --workspace --release` и `--profile dist` | Дополнительная объективная проверка D25 | **PASS** | Код возврата `0` (`release`: `0.05s`, `dist`: `0.04s`) |

---

## 2. Критический разбор предыдущей ревизии (`<prior_attempt>`)

В ходе независимой перепроверки выявлены 3 пробела в предыдущем отчете аудитора:

1. **Пробел 1 — Неверифицированная компиляция профилей `release` и `dist`:**
   - *Утверждение в `<prior_attempt>`:* Пункт 1.2 отмечен `PASS` только на основании визуального чтения строк `[Cargo.toml:62-84](file:///home/gidragir/projects/context-init/Cargo.toml#L62-L84)` и запуска `cargo check --workspace` (профиль `dev`) / `cargo nextest` (профиль `test`).
   - *Фактическая проверка:* Выполнены команды `rtk cargo check --workspace --release` и `rtk cargo check --workspace --profile dist`. Обе завершились с кодом `0` без предупреждений Cargo о синтаксисе кастомного профиля `dist` (`inherits = "release"`, `lto = "fat"`, `strip = "symbols"`, `panic = "abort"`).
2. **Пробел 2 — Несогласованность `[workspace.dependencies]` для внутренних крейтов:**
   - *Утверждение в `<prior_attempt>`:* Указано, что крейты наследуют зависимости из workspace и корректно ссылаются друг на друга.
   - *Что реально в коде:* В корневом [Cargo.toml:16-18](file:///home/gidragir/projects/context-init/Cargo.toml#L16-L18) объявлены workspace-зависимости `ci-core = { path = "crates/ci-core" }`, `ci-llm = { path = "crates/ci-llm" }`, `ci-mcp = { path = "crates/ci-mcp" }`, но **ни один манифест их не использует через `.workspace = true`**:
     - В [Cargo.toml:43-45](file:///home/gidragir/projects/context-init/Cargo.toml#L43-L45) повторно записаны пути `{ path = "crates/..." }`.
     - В [crates/ci-llm/Cargo.toml:9](file:///home/gidragir/projects/context-init/crates/ci-llm/Cargo.toml#L9) и [crates/ci-mcp/Cargo.toml:9-10](file:///home/gidragir/projects/context-init/crates/ci-mcp/Cargo.toml#L9-L10) используются относительные пути `{ path = "../ci-..." }`.
     - Кроме того, в `[workspace.dependencies]` у `ci-core`, `ci-llm`, `ci-mcp` не указано поле `version = "0.3.0"`, которое потребуется при настройке `release-plz` / `crates.io` (D27 / O2).
3. **Пробел 3 — Игнорирование сопутствующих изменений в `src/detector.rs`, `src/generator.rs` и `tests/regression_benchmark.rs`:**
   - *Утверждение в `<prior_attempt>`:* Рассмотрены только `Cargo.toml`, `crates/*` и `src/main.rs`.
   - *Что реально в коде:* При реализации Шага 1 были также модифицированы [src/detector.rs](file:///home/gidragir/projects/context-init/src/detector.rs), [src/generator.rs](file:///home/gidragir/projects/context-init/src/generator.rs) и [tests/regression_benchmark.rs](file:///home/gidragir/projects/context-init/tests/regression_benchmark.rs):
     - В [`parse_cargo_toml`](file:///home/gidragir/projects/context-init/src/detector.rs#L126-L150) добавлен парсинг через `toml::Value`, так как старый построчный парсер некорректно обрабатывал секции `[workspace.package]` и `version.workspace = true`.
     - В [`discover_subsystems`](file:///home/gidragir/projects/context-init/src/detector.rs#L592-L596) добавлен фильтр `if trimmed_member.is_empty() || trimmed_member == "." { continue; }`, предотвращающий генерацию пустого идентификатора подсистемы `""` для корневого члена workspace `"."`, а также объединение `src/main.rs` и `src/lib.rs` в единый модуль `src/core` ([src/detector.rs:688-735](file:///home/gidragir/projects/context-init/src/detector.rs#L688-L735)).
     - Добавлены 3 новых unit-теста в [src/detector.rs:1988-2117](file:///home/gidragir/projects/context-init/src/detector.rs#L1988-L2117).

---

## 3. Детальный технический анализ по пунктам чек-листа

### Пункт 1: Корневой [Cargo.toml](file:///home/gidragir/projects/context-init/Cargo.toml) — `PASS`

1. **Конфигурация Workspace ([Cargo.toml:1-14](file:///home/gidragir/projects/context-init/Cargo.toml#L1-L14)):**
   ```toml
   [workspace]
   resolver = "2"
   members = [
       "crates/ci-core",
       "crates/ci-llm",
       "crates/ci-mcp",
       ".",
   ]

   [workspace.package]
   version = "0.3.0"
   edition = "2021"
   authors = ["gidragir"]
   ```
   - Полное соответствие требованию `resolver = "2"` и списку `members`.
   - Метаданные пакета `context-init` ([Cargo.toml:35-40](file:///home/gidragir/projects/context-init/Cargo.toml#L35-L40)) наследуют `version.workspace = true`, `edition.workspace = true`, `authors.workspace = true`.

2. **Конфигурация профилей сборки ([Cargo.toml:62-84](file:///home/gidragir/projects/context-init/Cargo.toml#L62-L84)):**
   ```toml
   [profile.dev]
   opt-level = 0
   debug = "line-tables-only"

   [profile.dev.package."*"]
   opt-level = 2

   [profile.test]
   opt-level = 1
   debug = "line-tables-only"

   [profile.release]
   opt-level = 3
   lto = "thin"
   codegen-units = 16

   [profile.dist]
   inherits = "release"
   opt-level = 3
   lto = "fat"
   codegen-units = 1
   strip = "symbols"
   panic = "abort"
   ```
   - Построчно идентично спецификации D25 и разделу 1 [orchestration_architecture.md](file:///home/gidragir/.gemini/antigravity/brain/33f3fc15-fa35-49db-85ed-27e725f95875/orchestration_architecture.md#L12-L34).

---

### Пункт 2: Крейты Workspace (`crates/ci-core`, `crates/ci-llm`, `crates/ci-mcp`) — `PASS`

1. **`crates/ci-core`:**
   - Манифест: [crates/ci-core/Cargo.toml](file:///home/gidragir/projects/context-init/crates/ci-core/Cargo.toml#L1-L16) — использует `.workspace = true` для метаданных и зависимостей (`anyhow`, `dirs-next`, `schemars`, `serde`, `serde_json`, `sha2`, `toml`).
   - Исходный код: [crates/ci-core/src/lib.rs](file:///home/gidragir/projects/context-init/crates/ci-core/src/lib.rs#L1-L21) — экспортирует `pub const VERSION: &str = env!("CARGO_PKG_VERSION");`, функцию [`crate_info`](file:///home/gidragir/projects/context-init/crates/ci-core/src/lib.rs#L7-L9) и содержит unit-тест `smoke_version_matches_workspace`.

2. **`crates/ci-llm`:**
   - Манифест: [crates/ci-llm/Cargo.toml](file:///home/gidragir/projects/context-init/crates/ci-llm/Cargo.toml#L1-L14) — ссылается на `ci-core = { path = "../ci-core" }` и workspace-зависимости (`anyhow`, `serde`, `serde_json`, `ureq`).
   - Исходный код: [crates/ci-llm/src/lib.rs](file:///home/gidragir/projects/context-init/crates/ci-llm/src/lib.rs#L1-L22) — экспортирует `VERSION`, [`crate_info`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/lib.rs#L7-L9) (с проверкой связки с `ci_core::VERSION`) и unit-тест `smoke_version_and_core_link`.

3. **`crates/ci-mcp`:**
   - Манифест: [crates/ci-mcp/Cargo.toml](file:///home/gidragir/projects/context-init/crates/ci-mcp/Cargo.toml#L1-L18) — ссылается на `ci-core` и `ci-llm`, а также workspace-зависимости (`anyhow`, `rmcp`, `schemars`, `serde`, `serde_json`, `tiktoken-rs`, `tokio`).
   - Исходный код: [crates/ci-mcp/src/lib.rs](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/lib.rs#L1-L28) — экспортирует `VERSION`, [`crate_info`](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/lib.rs#L7-L14) и unit-тест `smoke_version_and_workspace_links`.

---

### Пункт 3: Проверка [src/main.rs](file:///home/gidragir/projects/context-init/src/main.rs) — `PASS`

1. **Атрибут версии Clap без строкового литерала (D27):**
   В [src/main.rs:8-15](file:///home/gidragir/projects/context-init/src/main.rs#L8-L15):
   ```rust
   #[derive(Parser, Debug)]
   #[command(
       name = "context-init",
       author = "gidragir",
       version,
       about = "🚀 Инициализация и поддержание трехуровневой базы знаний (L0-L2 OpenViking) для ИИ",
       long_about = "Сканирует проект, выявляет структуру монорепозиториев, точки входа, команды сборки/тестов и генерирует высокоэффективный навигационный контекст для ИИ-ассистентов."
   )]
   ```
   Покрыто unit-тестом [`test_cli_version_matches_cargo_pkg_version_and_workspace_crates`](file:///home/gidragir/projects/context-init/src/main.rs#L1002-L1011).

2. **Отсутствие дублирования `mod detector; mod generator; ...` в `src/main.rs`:**
   - В [src/main.rs:4](file:///home/gidragir/projects/context-init/src/main.rs#L4) используется библиотечный крейт:
     ```rust
     use context_init::{generator, mcp, service, types};
     ```
   - Модули объявлены ровно один раз в [src/lib.rs:1-6](file:///home/gidragir/projects/context-init/src/lib.rs#L1-L6).
   - Вывод `rtk rg -n '^\s*(pub\s+)?mod\s+' src/main.rs`:
     - Строка 608: `mod tests {` (тестовый модуль `src/main.rs`).
     - Строка 662: `mod tests {` (внутри строкового литерала фикстуры в тесте `test_compress_rust_ast_with_attributes_and_references`).

---

### Пункт 4: Проверка отсутствия упоминаний «1С» / «1C» — `PASS`

Команда:
```bash
rtk rg -i '1[cс]' Cargo.toml crates/ src/
```
Результат: код возврата `1`, `stdout` и `stderr` пусты (0 совпадений как с латинской `C`, так и с кириллической `С`).

---

### Пункт 5: Объективные проверки компиляции, линтера и тестов — `PASS`

1. **`rtk cargo check --workspace`**
   ```text
   Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.12s
   Exit code: 0
   ```
2. **`rtk cargo check -p ci-core -p ci-llm -p ci-mcp -p context-init`**
   ```text
   Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
   Exit code: 0
   ```
3. **`rtk cargo clippy --workspace --all-targets -- -D warnings`**
   ```text
   cargo clippy: No issues found
   Exit code: 0
   ```
4. **`rtk cargo nextest run --workspace`**
   ```text
   cargo nextest: 39 passed (6 binaries, 0.177s)
   Exit code: 0
   ```
5. **`rtk cargo check --workspace --release && rtk cargo check --workspace --profile dist`**
   ```text
   Finished `release` profile [optimized] target(s) in 0.05s
   Finished `dist` profile [optimized] target(s) in 0.04s
   Exit code: 0
   ```

---

## 4. Remaining Questions & Gaps

- **Унификация внутренних зависимостей через `.workspace = true` и `version` (D27 / O2):** В [Cargo.toml:16-18](file:///home/gidragir/projects/context-init/Cargo.toml#L16-L18) объявлены `ci-core`, `ci-llm`, `ci-mcp` в `[workspace.dependencies]`, однако в `[dependencies]` корневого пакета ([Cargo.toml:43-45](file:///home/gidragir/projects/context-init/Cargo.toml#L43-L45)) и в дочерних крейтах ([crates/ci-llm/Cargo.toml:9](file:///home/gidragir/projects/context-init/crates/ci-llm/Cargo.toml#L9), [crates/ci-mcp/Cargo.toml:9-10](file:///home/gidragir/projects/context-init/crates/ci-mcp/Cargo.toml#L9-L10)) указаны прямые `path`-зависимости вместо `.workspace = true`. При переходе к шагу настройки `release-plz` / `crates.io` (D27, O2 в [milestone_decisions.md:133](file:///home/gidragir/projects/context-init/milestone_decisions.md#L133)) рекомендуется перевести ссылки на `ci-*.workspace = true` и добавить `version = "0.3.0"` (а также при необходимости `package = "context-init-*"`) в единой секции `[workspace.dependencies]`.
- **Поэтапное выделение остальных крейтов D24:** В Шаге 1 созданы 3 целевых библиотечных крейта (`ci-core`, `ci-llm`, `ci-mcp`) для вехи v0.3. Остальные крейты из полного перечня D24 (`ci-scan`, `ci-lang`, `ci-git`, `ci-kb`, `ci-search`, `ci-telemetry`) относятся к следующим этапам декомпозиции.
- **Синхронизация базы знаний `.agents/knowledge` и установленного бинарника MCP:** Запуск `rtk cargo run --quiet -- check` показывает 4 новых неотслеживаемых модуля на диске (`crates_ci-core`, `crates_ci-llm`, `crates_ci-mcp`, `references`), тогда как запущенный в фоне старый хостовый процесс MCP-сервера (`context_check`) ещё не содержит патча [`discover_subsystems`](file:///home/gidragir/projects/context-init/src/detector.rs#L592-L596) и дополнительно выдает пустую строку `""` на элемент `"."` в `workspace.members`. Обновление базы знаний и пересборка бинарника должны выполняться на финальном шаге после наполнения крейтов.
