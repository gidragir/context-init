# context-init: Компактность L0, универсальная гранулярность детектора и защита `scan`/`update`

**Дата:** 2026-11-07  
**Статус:** Готов к утверждению  
**Основание:** Эксплуатационный аудит на мультирепозиторных задачах, ревизия кода `ci-core` / `ci-mcp` / `src/cli` и очистка ядра от частных топологических эвристик.

---

## 1. Назначение и мотивация (Intent & Why)

Анализ эксплуатации `context-init` на крупных и гетерогенных репозиториях выявил ряд **общесистемных дефектов ядра**, а также накопленный технический долг из частных эвристик в `scope.rs` и `detector.rs`:

1. **Нарушение контракта L0 в `context_get_subsystems`:** обработчик сериализует полные структуры `SubsystemInfo` со списками файлов, точек входа и AST-сигнатур (данные уровня L1), из-за чего даже на самом репозитории `context-init` (`33` подсистемы) ответ обрезается на 8-м элементе (`12.3 КБ` / `3 647` токенов) и сбрасывается рантаймом во внешний файл.
2. **Атомизация плоских папок и потеря файлов контейнеров в `detector.rs`:**
   - Вторичный цикл `SplitContext::split` при отсутствии code-entrypoint принудительно выселяет все файлы `>= 300` токенов в отдельные L1-подсистемы, разваливая плоские папки скриптов, миграций и тестов на десятки мелких карточек.
   - При обходе стандартных каталогов-контейнеров монорепо (`packages/`, `services/`, `apps/`, `infra/` и др.) наличие хотя бы одной подпапки приводит к полной потере прямых файлов верхнего уровня самого контейнера (`packages/tsconfig.base.json`, `infra/compose.yaml`).
3. **Коллизии идентификаторов в `sanitize_id`:** слепое усечение `.take(64)` без хеш-суффикса склеивает разные длинные пути в один `subsystem_id` и порождает дубли в `L0_index.json` и `AGENTS.md`.
4. **Сломанный обход директорий по `[scope].include` и частные костыли в `ScopeFilter`:** `WalkBuilder::filter_entry` проверяет `include.is_match(rel_dir)` на самом имени каталога и не заходит внутрь скрытых или игнорируемых папок при масках вида `.storybook/**` или `.config/**`; при этом в `scope.rs` и `detector.rs` остались частные хардкоды под конкретные раскладки каталогов (`is_stow_or_dotfiles`, `"linked"`, `"dot_config"`), которым не место в универсальном ядре.
5. **Уязвимость `context_scan` к «контекстным бомбам» (`Outliers`) и слепота на глубине > 1:** одиночные тяжёлые сгенерированные или вендорные файлы перетягивают `share_percent` и вытесняют весь реальный код из `suggested_scope.extensions`; префиксные манифесты с точками в имени (`Dockerfile.prod`, `Modelfile.qwen2.5`) дают мусорные расширения; срез директорий ограничен глубиной `1`.
6. **Отсутствие штатного прунинга удалённых корневых подсистем (`missing_in_fs`):** `generator::update` удаляет исчезнувшие дочерние сплиты, но вечно хранит удалённые корневые подсистемы, вынуждая нарушать правило `ZERO DIRECT FILE ACCESS`.
7. **Слепота `context_workspace` к статусу базы знаний:** при работе с несколькими репозиториями инструмент не показывает, где база инициализирована, актуальна или отсутствует, и не принимает явный список путей.

Цель спецификации — решить все перечисленные задачи **исключительно универсальными механизмами ядра**, одновременно вычистив из `ci-core` частные топологические хардкоды.

---

## 2. Цели, границы и анти-цели

### 2.1. Цели и критерии успеха (Goals)

| # | Критерий | Проверка |
| :---: | :--- | :--- |
| **S1** | **Истинный L0 в `context_get_subsystems`:** Ответ содержит только компактную L0-проекцию без массивов `files`, `key_files`, `entrypoints`, `key_symbols`. На индексе из `65` подсистем весь ответ укладывается в одну страницу (`< 4 000` токенов, `< 9 КБ`) без усечения. | Юнит-тест в `ci-mcp` на 65 подсистемах (`truncated == false`), прогон на `context-init`. |
| **S2** | **Универсальная гранулярность сплиттера и сохранность файлов контейнеров:** Одиночные файлы `< 1 000` токенов и одиночные тестовые файлы никогда не становятся L1-подсистемами; плоские каталоги без пакетного манифеста и модульного entrypoint делятся только по поддиректориям; прямые файлы контейнеров монорепо не теряются при наличии подпапок. | На `context-init` исчезают микроподсистема `mise` (`698` токенов) и пофайловые `tests_*`; юнит-тесты на плоский `scripts/` из 20 файлов и контейнер `packages/` со смешанным содержимым. |
| **S3** | **Бесколлизионные `subsystem_id`:** Пути длиннее 64 символов получают детерминированный суффикс `_{8hex}` (SHA-256 от полного исходного ключа); коллизии разных путей разрешаются автоматически; в `L0_index.json` и `AGENTS.md` исключены дубликаты `id`. | Юнит-тесты на 3 путях с общим 64-символьным префиксом и на паре `a/b_c` vs `a_b/c`. |
| **S4** | **Декларативный `ScopeFilter` и зачистка ядра от частных хардкодов:** Из `scope.rs`, `detector.rs`, `storage.rs` и `config.rs` удалена вся частная логика под конкретные менеджеры симлинков/дотфайлов; `ScopeFilter` корректно спускается в любые директории, являющиеся префиксом масок `[scope].include`. | Юнит-тест `[scope] include = [".storybook/**", ".config/**"]` находит файлы внутри скрытых папок без каких-либо специальных флагов в ядре. |
| **S5** | **Защита `context_scan` от Context Bombs и диагностика дерева:** `ScanReport` выделяет аномальные файлы (`Outliers`), предлагает их в `suggested_scope.exclude`, считает `extensions` без искажения выбросами, синхронно со `ScopeFilter` распознаёт составные шаблоны (`*.yaml.j2`, `*.toml.tmpl`) и префиксные манифесты (`Dockerfile.*`, `Makefile.*`) и поддерживает `max_depth` (по умолчанию `2`) и диагностический `--include-hidden`. | Юнит-тесты на фикстуре с 8 вендорными мега-файлами (95% токенов) и 20 рабочими скриптами, а также на `Dockerfile.v1.2` и `scan --include-hidden`. |
| **S6** | **Штатный прунинг `update --prune` / `prune: true`:** Удалённые с диска корневые подсистемы (`missing_in_fs`) удаляются из `L0_index.json`, `L0_index.md`, `subsystems/<id>.md` и `AGENTS.md` штатной командой; наличие legacy `.agents/knowledge/` возвращает явную подсказку миграции. | Интеграционный тест `init` → удаление корневого модуля → `update --prune` → `check` рапортует `0 drift`. |
| **S7** | **Мультирепо-сводка в `context_workspace`:** Поддержка списка `paths: Vec<String>` и быстрой колонки `kb_status` (`ok` / `drift` / `uninitialized` / `legacy` / `missing_path`) без полной BPE-токенизации репозиториев. | Юнит-тест `workspace` на наборе путей в разных состояниях базы и с совпадающими `basename`. |

### 2.2. Вне скоупа (Non-Goals)

- Миграция дисковой схемы `L0_index.json` (остаётся `CURRENT_L0_SCHEMA_VERSION = 4`).
- Специализированные профили, пресеты или эвристики под конкретные утилиты управления конфигурациями (Stow, Chezmoi, Ansible, Nix и др.).
- Автоматическое удаление карточек с ручными правками (`Manual`) при вызове `update` **без** явного флага `--prune`.

### 2.3. Анти-цели (Anti-Goals)

- **Никаких зашитых путей и имён под частные раскладки репозиториев в ядре:** ни `scope.rs`, ни `detector.rs` не должны содержать списков вида `"infra/stow"`, `"dot_config"`, `"linked"` и аналогичных частных соглашений.
- **Никакого автоматического включения скрытых dot-каталогов (`.agents`, `.gemini`, `.claude`, `.github` и др.) в `init`/`update` без явного `[scope].include` пользователя:** по умолчанию все dot-директории игнорируются; их обзор без изменения конфига доступен только в диагностическом `scan --include-hidden`.
- **Никакой полной BPE-токенизации внутри `context_workspace`:** глубокий подсчёт токенов остаётся задачей параллельных вызовов `context_scan`.

---

## 3. Ограничения и инварианты (Constraints)

1. **Обратная совместимость `L0_index.json` и `.context-init.toml`:** старые `L0_index.json` (содержащие пустой массив `"stow_packages": []`) и старые `.context-init.toml` (содержащие `stow = false` в секции `[integrations]` со строгим `deny_unknown_fields`) должны десериализоваться без ошибок.
2. **Валидность `subsystem_id`:** все идентификаторы строго удовлетворяют `SUBSYSTEM_ID_PATTERN` (`^[a-z0-9][a-z0-9_-]{0,63}$`).
3. **Единство фильтрации (`D9`):** `scan` (в штатном режиме), `init`, `update` и `pack` используют один и тот же `ScopeFilter` и один и тот же классификатор расширений `canonical_file_ext`.
4. **Безусловная защита от обхода `.git` и симлинков:** `follow_links(false)` неизменен; каталог `.git` блокируется на любой глубине даже при явном `[scope].include` и `include_hidden = true`.

---

## 4. Архитектурное решение (Approaches)

Вместо введения профилей топологий под частные виды репозиториев выбран **путь обобщения базовых алгоритмов ядра**:

| Задача | Частный подход (отклонён) | Универсальный механизм ядра (принят) |
| :--- | :--- | :--- |
| Включение нестандартных/скрытых папок в проект | Зашивать белые списки dot-папок и профили в `ScopeFilter` | Починить префиксный обход директорий для стандартного `[scope].include` в `ScopeFilter::build` + дать обзор через `scan --include-hidden` |
| Потеря верхнеуровневых файлов в папках с подмодулями | Спец-правила для отдельных конфиг-папок | Универсальный сбор остаточных прямых файлов для **любого** контейнера монорепо в `discover_subsystems` |
| Дробление папок со скриптами на десятки карточек | Хардкод списков скриптовых папок | Удаление каскадного выселения мелких файлов + структурный критерий плоского каталога без пакетного манифеста и модульного entrypoint + порог `>= 1 000` токенов |

---

## 5. Детальное проектирование

Реализация выполняется в **3 волны** (P0 → P1 → P2).

### Волна 1 (P0): Контракт L0, универсальный сплиттер, контейнеры и бесколлизионные ID

#### 5.1. Чистая L0-проекция в `ContextHandler::get_subsystems`

В `crates/ci-mcp/src/tools.rs` добавляется выходная DTO-структура L0:

```rust
/// Compact L0 projection of one subsystem returned by `context_get_subsystems`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct L0SubsystemSummary {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    pub file_count: usize,
    pub tokens: usize,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stale: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub needs_review: bool,
}
```

**Изменения в `ContextHandler::get_subsystems` (`crates/ci-mcp/src/handlers.rs`):**
1. По уже полученному `RepoGitSnapshot` флаг `stale` вычисляется для каждой подсистемы без дополнительных вызовов `git` (убирается ранний `break` в цикле проверки устаревания).
2. Из ответа полностью исключаются L1-поля `files`, `key_files`, `entrypoints`, `key_symbols`, `l1_path`, `test_command`, `summary_source` (вместо массива `files` передаётся скаляр `file_count: sub.files.len()`).
3. Ответ форматируется как валидный JSON-массив, где каждая запись `L0SubsystemSummary` сериализуется в одну компактную строку (`[\n  {...},\n  {...}\n]`), что сокращает объём выдачи в **6–8 раз** и сохраняет обратную совместимость с JSON-парсерами.

#### 5.2. Универсальные правила гранулярности и контейнеров в `detector.rs`

1. **Порог одиночной файловой подсистемы:**
   ```rust
   pub const MIN_SINGLE_FILE_SUBSYSTEM_TOKENS: usize = 1_000;
   ```
   В `SplitContext::split` одиночный файл (`entry.dir.is_none()`) с `entry.tokens < MIN_SINGLE_FILE_SUBSYSTEM_TOKENS` никогда не промоутится в самостоятельную L1-подсистему.
2. **Удаление каскадного выселения мелких файлов:**
   Вторичный цикл `if has_promoted_file && !has_pinned_code_entrypoint` (`detector.rs:1468–1489`), который принудительно превращал все оставшиеся файлы каталога `>= 300` токенов в отдельные подсистемы даже после достижения `kept_tokens <= threshold`, **удаляется**. Промоушен выполняется исключительно до тех пор, пока `kept_tokens > self.threshold`.
3. **Структурный критерий плоских файловых коллекций:**
   Каталог `dir` считается плоской коллекцией без модульной иерархии (`is_flat_file_collection`), если одновременно:
   - в `dir` нет пакетного манифеста (`Cargo.toml`, `package.json`, `pyproject.toml`, `go.mod`, `build.gradle`, `build.gradle.kts`, `pom.xml`, `CMakeLists.txt`), нет подкаталога `src/` и нет соседнего одноимённого файла-модуля (`<dir>.rs`);
   - ни один из его прямых файлов не является модульным кодовым entrypoint (`main.*`, `lib.rs`, `mod.rs`, `index.*`, `app.*`, `server.*`, `__init__.py`).
   Для такого каталога (например, `scripts/`, `migrations/`, `hooks/`, `examples/`, `sql/`) `SplitContext::split` при превышении `split_threshold` может промоутировать **только вложенные поддиректории** (`entry.dir.is_some()`), но не расщепляет одиночные файлы (`entry.dir.is_none()`), оставляя плоский набор файлов единой подсистемой с выборкой до 15 `representative_files`.
4. **Неатомарность тестов на всех уровнях:**
   В `SplitContext::split` (`detector.rs:1438`) убирается исключение `id != "tests"`: одиночный тестовый файл (`entry.dir.is_none() && is_test_path`) никогда не становится отдельной L1-карточкой, включая корневой каталог `tests/`.
5. **Сохранение собственных файлов в каталогах-контейнерах монорепо (`discover_subsystems`):**
   - Из массива `common_parent` (`detector.rs:866–896`) удаляются частные имена `"profiles"`, `"home"`, `"linked"`, `"dot_config"`; остаются универсальные контейнеры монорепо: `&["crates", "packages", "apps", "services", "modules", "plugins", "internal", "cmd", "pkg", "libs", "infra", "submodules"]`.
   - Если контейнер содержит дочерние поддиректории-модули (`has_subdirs == true`) **и** одновременно содержит собственные релевантные файлы непосредственно на своём верхнем уровне (`depth == 1` внутри контейнера, например `packages/tsconfig.base.json`, `infra/compose.yaml`, `services/Makefile`), эти прямые файлы не отбрасываются, а формируют самостоятельный модуль контейнера (собирающий только прямые файлы контейнера и подпапки, не выделенные в отдельные подсистемы, без дублирования файлов дочерних модулей).

#### 5.3. Бесколлизионный `sanitize_id` и дедупликация индекса

1. **Детерминированный хеш-суффикс (`generator.rs:sanitize_id`):**
   - При длине очищенного идентификатора `<= 64` символов результат возвращается без изменений.
   - При длине `> 64` символов берётся префикс из первых `55` символов (`.trim_end_matches(['_', '-'])`) и дописывается `_{8hex}` — первые 4 байта `SHA-256` от полного исходного `raw` (итоговая длина `<= 64`).
2. **Разрешение коллизий разных путей:**
   - В финале `discover_subsystems`: если два модуля с разными `dir_path` получили одинаковый `id` (например, `a/b_c` и `a_b/c`), конфликтующим элементам назначается `id` с суффиксом `_{8hex(dir_path)}`.
3. **Защита индекса и `AGENTS.md`:**
   - В `normalize_index` и `render_root_agents_md` добавляется дедупликация по `id` (при схлопывании дублей в старом индексе приоритет отдаётся записи с не-`Heuristic` `summary_source`).

---

### Волна 2 (P1): Очистка и обобщение `ScopeFilter`, харденинг `context_scan` и `update --prune`

#### 5.4. Очистка от частного хардкода и префиксный обход `[scope].include` в `ScopeFilter`

1. **Удаление частного техдолга:**
   - Из `crates/ci-core/src/analysis/scope.rs` полностью удаляется функция `is_stow_or_dotfiles` и её ветка в `ScopeFilter::build`.
   - Из `crates/ci-core/src/storage.rs` удаляется неиспользуемое поле `L0Index::stow_packages` (старые JSON с `"stow_packages": []` продолжают читаться штатно).
   - Из `crates/ci-core/src/config.rs` удаляются публичное поле `IntegrationsConfig::stow`, `CliOverrides::stow`, переменные окружения `CONTEXT_INIT_STOW` / `CONTEXT_INIT_INTEGRATIONS_STOW` и строка `stow = false` в шаблоне `config init`. В приватной структуре `PartialIntegrationsConfig` (`#[serde(deny_unknown_fields)]`) сохраняется поле `#[serde(rename = "stow", default)] _legacy_stow: Option<bool>`, чтобы ранее созданные `.context-init.toml` проходили валидацию без ошибок.
2. **Универсальный спуск в директории по префиксу `[scope].include`:**
   - В `ScopeFilter::build` исправляется логика `explicitly_included` для директорий в `WalkBuilder::filter_entry`: помимо `include.is_match(rel_dir)`, проверяется, может ли `rel_dir` содержать файлы, подпадающие под паттерны `scope.include` (`include_allows_dir_traversal(rel_dir, &scope.include)`):
     - если паттерн в `scope.include` начинается с глобального джокера (`**/*`, `*.rs`), он **не** открывает скрытые dot-каталоги и `IGNORED_DIRS`;
     - если паттерн явно адресует конкретный путь или префикс (например, `.storybook/**`, `.config/nvim/**`, `vendor/custom/**`), то каждый родительский сегмент этого пути (`.storybook`, `.config`, `.config/nvim`, `vendor`, `vendor/custom`) пропускается через `filter_entry`, позволяя обходчику дойти до целевых файлов.
   - Исключение: каталог `.git` блокируется безусловно при любых значениях `scope.include`.

#### 5.5. Универсальный классификатор расширений и манифестов (`analysis/file_kind.rs`)

Создаётся чистый модуль `crates/ci-core/src/analysis/file_kind.rs`, используемый совместно в `scan.rs`, `scope.rs` и `detector.rs`:

```rust
/// Canonical extension or manifest key for `scan` tallies and `[scope].extensions`.
pub fn canonical_file_ext(rel_path: &str) -> Option<String>
```

1. **Стандартные префиксные манифесты с точками в суффиксе варианта** (регистронезависимо по `file_name`):
   - `Dockerfile` / `Dockerfile.*` / `Containerfile` / `Containerfile.*` → `"dockerfile"`
   - `Makefile` / `Makefile.*` / `GNUmakefile` → `"makefile"`
   - `Justfile` / `justfile` / `Justfile.*` / `*.just` → `"just"`
   - `CMakeLists.txt` → `"cmake"`
   - `Modelfile` / `Modelfile.*` → `"modelfile"`
   *(исключает появление псевдо-расширений вроде `.prod` или `.5-ctx64k` из суффиксов версий/окружений).*
2. **Составные шаблонные суффиксы** (`.tmpl`, `.j2`, `.jinja`, `.jinja2`, `.gotmpl`, `.in`, `.example`):
   - Для файлов вида `<stem>.<inner>.<wrapper>`, где `<inner>` входит в `CODE_EXTENSIONS`, возвращается `"<inner>.<wrapper>"` (например, `"yaml.j2"`, `"toml.tmpl"`, `"sh.in"`).
   - Иначе возвращается `<wrapper>`.
3. **Обычные файлы:** нижний регистр последнего расширения `Path::extension()`, либо `None` (в отчёте `scan` отображается как `"none"` и не попадает в `suggested_scope.extensions`).
4. **Согласованность со `ScopeFilter::build`:** при непустом `scope.extensions` файл проходит фильтр, если в `extensions` содержится его `canonical_file_ext(rel)` **или** его последний суффикс `Path::extension()` (полная обратная совместимость с простыми расширениями вида `"tmpl"` и `"j2"`).

#### 5.6. Детектор выбросов (`Outliers`), `max_depth` и `--include-hidden` в `context_scan`

В `crates/ci-core/src/analysis/scan.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanOptions {
    /// Directory aggregation depth (1..=5, default: 2).
    pub max_depth: usize,
    /// Diagnostic mode: include hidden dot-directories (except runtime caches and .git).
    pub include_hidden: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self { max_depth: 2, include_hidden: false }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScanOutlier {
    pub path: String,
    pub lines: usize,
    pub tokens: usize,
    pub share_percent: f64,
}
```

1. **Выявление Context Bombs (`ScanReport.outliers`):**
   - Файл попадает в выбросы, если `tokens >= 10_000` **и** (`tokens >= 25_000` **или** (`total_files >= 8 && share_percent >= 15.0`)). В отчёт включаются топ-10 файлов по убыванию `tokens` (секция `## Outliers (Context Bombs)`).
2. **Устойчивый расчёт `suggested_scope`:**
   - Расчёт `suggested_scope` ведётся строго по файлам, проходящим штатный `ScopeFilter` (даже при `include_hidden = true` диагностически раскрытые скрытые папки не засоряют рекомендуемый `[scope]`).
   - Подкаталоги глубины `1..=3`, где `>= 80%` токенов составляют файлы-выбросы и нет корневых манифестов сборки, добавляются в `suggested_scope.exclude` как `<dir>/**`; одиночные выбросы вне таких папок добавляются точным путём файла.
   - Доли расширений для `suggested_scope.extensions` вычисляются по `clean_total_tokens` (после вычета `outliers`): расширение включается при `clean_share_percent >= 1.0%` **или** (`clean_files >= 3 && clean_tokens > 0`).
3. **Глубина дерева `max_depth` и диагностика `--include-hidden`:**
   - `ScanReport.directories` сохраняет 1-й уровень (сумма долей `100%`), а новое поле `ScanReport.subdirectories: Vec<ScanStat>` агрегирует пути глубины `2..=max_depth` (с долей `>= 2.0%`, топ-20 по токенам), отображаемые в `render_scan_markdown` под соответствующими родительскими директориями.
   - При `include_hidden = true` обходчик в `scan` пропускает скрытые dot-каталоги (`.github`, `.agents`, `.storybook`, `.config` и др.), оставляя заблокированными только `.git` и runtime/build-кэши (`node_modules`, `target`, `.venv`, `.context-init`, `.cache`, `.worktrees`, `.claude/worktrees` и др.).
   - Параметры добавляются в CLI `ScanArgs` (`--max-depth`, `--include-hidden`) и MCP `McpScanParams` (`max_depth`, `include_hidden`).

#### 5.7. Штатный прунинг `update --prune` и подсказка миграции `.agents/knowledge`

1. **Флаг `prune`:**
   - CLI `UpdateArgs`: `--prune`; MCP `McpUpdateParams`: `prune: Option<bool>`; `PipelineOptions`: `prune: bool`; `PipelineOutcome`: `pruned: usize`.
   - В `generator::update_with_options(..., prune: bool)`: при `prune = true` все существующие записи `index.subsystems`, не сматчившиеся ни с одним актуальным модулем на диске (включая корневые `parent.is_none()`), удаляются из индекса, их файлы `subsystems/<id>.md` удаляются с диска, а `L0_index.json`, `L0_index.md` и `AGENTS.md` перезаписываются. При `prune = false` сохраняется текущее консервативное поведение.
2. **Подсказки и документация скилла:**
   - При наличии `missing_in_fs` в `check` к сообщению добавляется подсказка `use update --prune / prune=true`.
   - В `pipeline::storage_mismatch_hint` добавляется проверка legacy-пути `<root>/.agents/knowledge/L0_index.json` с подсказкой миграции в `.context-init/knowledge/`.
   - В `.agents/skills/context-init/SKILL.md` описывается штатная очистка через `context_update(prune=true)` / `rtk context-init update . --prune --compact` без прямого редактирования файлов базы.

---

### Волна 3 (P2): Пакетная сводка состояния баз в `context_workspace`

#### 5.8. Расширение `context_workspace` (`paths` + `kb_status`)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum WorkspaceKbStatus {
    Ok { subsystems: usize },
    Drift {
        subsystems: usize,
        missing: usize,
        untracked: usize,
        git_stale: bool,
    },
    LegacyAgentsDir,
    Uninitialized,
    MissingPath,
}
```

1. В `McpWorkspaceParams` добавляется `paths: Option<Vec<String>>`, в CLI `WorkspaceArgs` — повторяемый `--repo <PATH>`. При передаче списка анализируются указанные пути (с дедупликацией по `canonicalize`), иначе — авто-поиск Git-репозиториев внутри `path` (глубина 1–2).
2. Для каждого пути вычисляется `kb_status` без полной BPE-токенизации дерева (несуществующий путь получает `MissingPath`, существующая не-Git папка — `branch = "-"` и реальный `kb_status`, коллизии одинаковых `basename` уникализируются родительским сегментом `parent/name`).
3. `render_workspace_l0` выводит сводную таблицу `Repository | Branch | Dirty | KB Status | Subsystems` + секцию общих символов `Cross-Repository Shared Symbols`.

---

## 6. На усмотрение разработчика при реализации (Left to the Builder)

1. Внутренняя организация хелперов префиксного матчинга глобов в `scope.rs` и разбиение `file_kind.rs`.
2. Передача вычислителя `kb_status` между `ci-mcp` и `ci-core::analysis::workspace` (через замыкание или лёгкую функцию в `ci-core`).
3. Формат отступов для `subdirectories` и таблицы `outliers` в `render_scan_markdown` при сохранении заголовков существующих секций.

---

## 7. План верификации

1. **Юнит-тесты `ci-core`:**
   - `scope`: префиксный спуск в скрытые директории по `[scope] include = [".storybook/**", ".config/app/**"]`, закрытие скрытых папок при общем глобе `include = ["**/*.toml"]`, безусловная блокировка `.git`, совместимость старого `.context-init.toml` с `stow = false`.
   - `file_kind` + `scan`: классификация `Dockerfile.prod` / `Modelfile.qwen2.5` / `config.yaml.j2`, расчёт `Outliers` и очищенного `suggested_scope`, `max_depth = 2` и изоляция `suggested_scope` при `include_hidden = true`.
   - `detector` + `generator`: порог `MIN_SINGLE_FILE_SUBSYSTEM_TOKENS = 1_000`, неделимость плоского `scripts/` на 20 файлов, сохранение прямых файлов контейнера `packages/` при наличии `packages/app-a/`, запрет одиночных тестовых L1-карточек в корневом `tests/`, хеш-суффикс `sanitize_id` (`> 64` символов) и разрешение коллизий, `update_with_options(..., prune = true)`.
2. **Юнит- и интеграционные тесты `ci-mcp` и `tests/cli.rs`:**
   - Проверка размера и схемы `context_get_subsystems` на 65 подсистемах (`truncated == false`, отсутствие `files` и `key_symbols`, наличие `file_count`, `tokens`, `stale`).
   - Сквозной тест CLI `update --prune` и MCP `context_update(prune=true)`.
   - Сквозной тест `context_workspace` с `paths` и всеми состояниями `WorkspaceKbStatus`.
3. **Самопроверка на репозитории `context-init`:**
   - `rtk cargo test` (все тесты workspace зелёные).
   - Прогон `rtk cargo run -- update . --prune --compact` и `rtk cargo run -- check . --compact` (`0 drift`, удаление сиротской `tests_regression_benchmark` и микроподсистемы `mise`).
