# context-init: Компактная упаковка контекста, L1.5 TOC, бюджет CLI и ускорение синхронизации

**Дата:** 2026-11-15  
**Статус:** Готов к утверждению (прошёл независимое техническое ревью)  
**Основание:** Эксплуатационный отчёт по использованию `context-init` субагентами в Kotlin-монорепозитории `mimir`, исследовательский аудит [`2026-11-15-context-init-improvements-research.md`](file:///home/gidragir/projects/context-init/reports/subagents/2026-11-15-context-init-improvements-research.md) и независимое ревью [`2026-11-15-context-init-improvements-review.md`](file:///home/gidragir/projects/context-init/reports/subagents/2026-11-15-context-init-improvements-review.md).

---

## 1. Назначение и мотивация (Intent & Why)

Анализ логов реальной сессии субагентов в проекте `mimir` и аудит кодовой базы `context-init` (`crates/ci-core`, `crates/ci-mcp`, `src/cli`) выявили две взаимосвязанные группы проблем, снижающих ценность инструмента и принуждающих агентов откатываться к десяткам прямых чтений файлов (`48` и `73` вызова `read_file` в `mimir`):

1. **Потери токенов и обрывы навигации при упаковке контекста (Направление A):**
   - **Шум дефолтного XML (`PackStyle::Xml`):** в [`pack.rs:11-26`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/pack.rs#L11-L26), [`server.rs:474`](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/server.rs#L474) и [`src/cli/pack.rs:37`](file:///home/gidragir/projects/context-init/src/cli/pack.rs#L37) значением по умолчанию остаётся `"xml"`. Обёртка `<file path="...">\n<![CDATA[...]]>\n</file>` не экранирует `]]>`/атрибуты и расходует полезный лимит `DEFAULT_MAX_RESPONSE_TOKENS = 4000`, а языковые адаптеры сохраняют все `import`/`use`/`package`, комментарии, приватные функции, 3-строчные заглушки `⋮----` и целиком `#[cfg(test)] mod tests` (до `42.7%` строк в `pack.rs` и `48.6%` в `generator.rs`). В `BraceLangAdapter::extract_symbols` ([`brace.rs:21-23`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/lang/brace.rs#L21-L23)) жёстко зашит `"ts"` для Go/Java/C#, а `method_bodies` пуст в `brace.rs` и `kotlin.rs`.
   - **Отсутствие точечного drill-down (`files`, `symbols`) и уровня L1.5 (`toc`):** `McpPackParams` ([`tools.rs:90-108`](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/tools.rs#L90-L108)) и `PackArgs` ([`src/cli/pack.rs:18-39`](file:///home/gidragir/projects/context-init/src/cli/pack.rs#L18-L39)) работают только по всей подсистеме целиком. Получив скелет с вырезанными телами, агент не может догрузить 2–3 целевых файла с телами за 1 запрос и не имеет индекса с номерами строк `[Lstart-Lend]`.
   - **Повреждение вывода в CLI (`run_command`):** `src/cli/pack.rs:107` пишет неограниченный поток в `stdout`, из-за чего терминальные обёртки вырезают середину XML при превышении ~100 строк (`9/9` вызовов в отчёте `mimir`).
   - **Пассивная пагинация и lazy-MCP оверхед:** `format_footer` ([`pagination.rs:117-133`](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/pagination.rs#L117-L133)) не сообщает долю прогресса, оставшиеся файлы и формат следующего вызова; `CursorData` не привязан к хешу запроса/текста; `ContextInitServer::get_info` ([`server.rs:695-703`](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/server.rs#L695-L703)) не заполняет `instructions`, а `rule_template.md` / `SKILL.md` не дают готовых JSON-сигнатур параметров и устарели (`.agents/knowledge/**` в `SKILL.md:20`, `-o /tmp/context.xml`).

2. **Избыточный I/O, N+1 Git-подпроцессы и повторный BPE-пересчёт при синхронизации (Направление B):**
   - **N+1 форков `git`:** `generator::update_with_options` создаёт `SystemGitBackend::new()` и вызывает `git status` + `git rev-parse` + `git diff` в цикле для каждой неизменённой карточки; `run_check` вызывает `git rev-parse` на каждую подсистему и дважды читает `L0_index.json` и L1-карточки.
   - **Повторное чтение ФС и игнорирование `ScopeFilter`:** `scan.rs` дважды запускает `ignore::WalkBuilder` и делает лишний `File::open` в `is_likely_binary`; `detector.rs` использует `scope.files()` только для 1С `mdo`, а для остальных проектов заново рекурсивно вызывает `fs::read_dir`, читает каждый файл несколько раз (`count_lines`, `estimate_tokens_fast`, два раздельных `FileMemo`), а `subsystems::build_deterministic` и `handlers::get_subsystems` (дважды при активной телеметрии!) пересчитывают однопоточный `cl100k_base`.
   - **Узкие места в `refresh_stale_card` и `paginate_content`:** автообновление одной устаревшей карточки запускает полный `detect(project, false)` по всему репозиторию, а `paginate_content` вызывает `count_tokens` `10–30` раз в бинарном поиске на одну страницу.

---

## 2. Цели, не-цели и ограничения

### 2.1. Цели и критерии приёмки (Goals)

| ID | Цель | Критерий проверки |
| :---: | :--- | :--- |
| **G1** | **Компактный формат и контролируемое сжатие (`PackStyle::Compact` + `CompressOptions`):** удаление `<![CDATA[...]]>`, схлопывание блоков `import`/`use`/`package`, удаление `#[cfg(test)]`, обычных комментариев и однострочное `{ ⋮ }` без поломки legacy-контракта `compress_source`. | Экономия **≥ 40%–55%** токенов на сжатой подсистеме Rust/Kotlin/TS относительно текущего `xml + compress`; `compress_source(code, ext)` сохраняет обратную совместимость для существующих тестов. |
| **G2** | **Точечная выборка (`files`, `symbols`, `mode`):** фильтрация кандидатов подсистемы (и `"all"` с непустым `files`) до чтения с диска и до проверки `PACK_ALL_MAX_PAGES`, с возможностью выгрузить полные тела целевых файлов/методов за 1 вызов. | Юнит- и интеграционные тесты в `ci-core` / `ci-mcp` / `tests/cli.rs` на фильтрацию `files`, `symbols`, явную ошибку при 0 совпадений и строгие security-отказы. |
| **G3** | **Необрезаемый индекс уровня L1.5 (`PackMode::Toc` / `--toc`):** компактный построчный индекс файлов и публичных символов с точными диапазонами строк `[Lstart-Lend]` и компактным fallback для не-кодовых файлов (`.toml`, `.md`, `.json`, `.yaml`, `.sql`, `.sh`). | Типовая подсистема (10–25 файлов) укладывается в `~300–600` токенов и `< 100` строк (`truncated == false`). |
| **G4** | **Управляемый бюджет CLI `stdout` (`--max-lines`, `--max-tokens`, `--cursor`):** детерминированная лестница деградации `full -> compact -> toc -> paginate` с резервированием места под закрытие code fence и подсказку `--cursor`. | Вывод `rtk context-init pack` при `--max-lines 90` никогда не превышает 90 строк и не теряет хвост/закрытие блока кода. |
| **G5** | **Эргономика пагинации и онбординг агентов:** адаптивный `format_footer` (`compact` при `budget < 256`, расширенный с `page X/Y`, `remaining` и `next:` при `budget >= 256`), защита `CursorData.query_hash`, `ServerConfig.instructions`, актуальные `rule_template.md` (`<= 25` строк), `render_root_agents_md` и `SKILL.md`. | Все тесты `pagination.rs` и `ci-mcp/src/lib.rs` на `budget = 60..64` проходят без изменений лимитов; устаревшие курсоры отклоняются. |
| **G6** | **Батчинг Git и единый проход по ФС:** 1x `RepoGitSnapshot` с `git ls-tree -r -t -z HEAD` и `HEAD^{tree}` + сохранение паритета для unborn `HEAD`, `"."`, single-file `hash-object` и подмодулей; 1x `WalkBuilder`; явный `&ScopeFilter` в `detector.rs`; единый in-memory `AnalyzedFileCache` + внешний дисковый кэш метрик в `StoragePaths::cache_dir`. | В `update` / `check` число форков `git` сокращается с $O(N)$ до $O(1)$ (плюс группировка `diff_name_only` по уникальным `prev_commit`); каждый файл читается с диска не более 1 раза за запуск. |
| **G7** | **Устранение лишнего BPE и быстрая пагинация:** передача предвычисленных токенов в `subsystems::build_deterministic`, устранение двойного `live_source_tokens` в `handlers.rs`, UTF-8-безопасный срез ограниченного байтового окна в `pagination.rs` и точечный `refresh_stale_card` с fallback для `.mdo` и split-крейтов. | Суммарное время `check` / `update` / `scan` на среднем и крупном монорепо снижается кратно без нарушения побайтового детерминизма `L0_index.json` и L1-карточек. |

### 2.2. Вне скоупа и отклонённые решения (Non-Goals & Rejected)

Следующие предложения из первичного исследования **сознательно отклонены или отложены** по итогам независимого ревью:

1. **Отклонено: снятие `#[serde(skip)]` с `SubsystemInfo::tokens` и `SubsystemFile::lines` в `L0_index.json`.**  
   Тест `l0_omits_volatile_fields` ([`analysis/mod.rs:109-139`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/mod.rs#L109-L139)) прямо запрещает запись `"tokens"`, `"lines"`, `"root_path"` в коммиченный `L0_index.json` во избежание шума в Git diff. Кэширование выносится исключительно в системный каталог `storage_paths.cache_dir` ([`config.rs:45`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L45)), вне репозитория.
2. **Отклонено: добавление крейта `blake3`.**  
   В [`Cargo.toml:34`](file:///home/gidragir/projects/context-init/Cargo.toml#L34) уже подключён `sha2 = "0.10"`, который используется для хеширования контента и `paths_tree_sha`. Новая зависимость не нужна.
3. **Отклонено: широкие маски `*secret*` и `*credential*` в `is_secret_path`.**  
   Подстрочный wildcard заблокировал бы легитимный исходный код (`secret_rotation.rs`, `CredentialsProvider.kt`, `test_secrets.py`). Вместо этого добавляется точный перечень секретных имён и расширений.
4. **Отложено: `rayon` в `detector.rs` и `syn/span-locations` в `Cargo.toml`.**  
   - `detector::tokens_if_over` ([`detector.rs:1241-1253`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/detector.rs#L1241-L1253)) уже распараллелен через `std::thread::scope`, тогда как `thread_local! SCOPE` ([`detector.rs:88-114`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/detector.rs#L88-L114)) сломал бы фильтрацию `.gitignore`/`[scope]` в рабочих потоках `rayon`. Сначала `thread_local! SCOPE` заменяется явным `&ScopeFilter`, а необходимость `rayon` оценивается по профилированию после устранения повторного I/O и BPE.
   - Для Rust `SymbolSpan` сначала проектируется 1-проходный сканер таблицы смещений строк и AST/лексических границ без глобального включения `proc-macro2/span-locations`, чтобы не утяжелять компиляцию.

### 2.3. Ограничения и инварианты (Constraints)

1. **Схема и инварианты KB:** `CURRENT_L0_SCHEMA_VERSION = 4` и тест `l0_omits_volatile_fields` остаются незыблемыми; файл кэша метрик никогда не создаётся внутри рабочего дерева репозитория (чтобы не загрязнять `git status`).
2. **Безопасность путей и XML:** фильтрация `files` проверяет только белый список файлов подсистемы (или индекса при `all`), отклоняет `/`, `\`, Windows drive-префиксы `[A-Za-z]:`, `..`, NUL-байты, проверяет `!symlink_metadata()?.file_type().is_symlink()` и `canonical_path.starts_with(&canonical_root)` в `pack_subsystem` и `pack_files`; XML-вывод обязан экранировать `]]>` внутри `<![CDATA[...]]>` и атрибуты `id`/`name`/`path`.
3. **Детерминизм:** все обходы, группировки и сериализации используют `BTreeMap` / `BTreeSet` и стабильную сортировку, а все записываемые Markdown/JSON-файлы оканчиваются `\n`.
4. **Бюджет пагинации и размер `AGENTS.md`:** футер при `budget < 256` (включая `budget = 60` и `MIN_RESPONSE_TOKENS = 64`) обязан укладываться в `footer_reserve <= 35` токенов и `token_count <= effective_max_tokens`; длина `rule_template.md` строго `<= 25` строк; корневой `AGENTS.md` обязан оставаться `<= 22 000` байт (текущий размер — `6014` байт).
5. **Синхронность документации и флагов:** обновления `rule_template.md`, `render_root_agents_md` и `SKILL.md` вносятся строго после или в одном коммите с появлением рекламируемых флагов (`--style compact`, `--toc`, `--files`, `--max-lines`).

---

## 3. Архитектурные решения и компромиссы (Approaches)

| Вопрос проектирования | Вариант A (отклонён) | Выбранное решение | Обоснование выбора |
| :--- | :--- | :--- | :--- |
| **Смена дефолта `Xml` -> `Compact` и совместимость `compress_source`** | Поменять `compress_source` и `PackStyle::parse` напрямую | Сохранить `compress_source(content, ext)` в legacy-режиме (`CompressOptions::legacy()`, `⋮----`); ввести `compress_source_with(..., &CompressOptions)`; добавить `PackStyle::try_parse` со строгой ошибкой на неизвестный стиль | Сохраняет совместимость unit-тестов ядра, исключает молчаливый откат при опечатке `--style` и делает переход дефолта `"compact"` управляемым и явным. |
| **Кэширование вычисленных токенов и строк** | Убрать `#[serde(skip)]` в `L0_index.json` | Передавать `AnalyzedFileCache` / `BTreeMap<String, usize>` в памяти между этапами (`detector` -> `subsystems` -> `handlers`), а на диск сохранять только в `StoragePaths::cache_dir/<repo_hash>/metrics_v1.json` | Полностью убирает повторный BPE в одном запуске и между холодными CLI-вызовами без нарушения `l0_omits_volatile_fields` и загрязнения Git. |
| **Улучшенный `format_footer` при малых бюджетах** | Безусловно выводить длинный футер (~65–85 токенов) | Двухуровневый рендер: при `effective_max_tokens < 256` — компактный формат `[truncated: true \| next_cursor: "cur_..." \| stale: false \| tokens: N]` (или `+ page: 1/K` при попадании в `<= 35` ток.); при `>= 256` — расширенный с `remaining` и `next:` | Не ломает `MIN_RESPONSE_TOKENS = 64` и существующие тесты парсинга футера, но даёт максимум навигации в реальных MCP-ответах (`budget = 4000`). |
| **Вычисление `[Lstart-Lend]` для Rust `SymbolSpan`** | Включить `syn/span-locations` глобально | Сначала лексический однопроходный парсер/индексатор строк поверх AST/файла (с fallback по сигнатурам); `syn/span-locations` — только если лексический проход окажется недостаточным по точности | Нулевой прирост времени сборки `proc-macro2` и независимость от proc-macro feature unification. |

---

## 4. Детальное проектирование

Реализация разбивается на **3 последовательные волны** (Wave 1 → Wave 2 → Wave 3), чтобы исключить поломку сборки и рассинхрон шаблонов инструкций с реальными флагами CLI/MCP.

### Волна 1: Контракт упаковки, безопасность путей, фильтрация (`files`/`symbols`) и адаптивная пагинация

#### 1.1. `PackStyle`, `PackMode`, `CompressOptions` и `PackRequest` (`crates/ci-core/src/analysis/pack.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackStyle {
    #[default]
    Compact,
    Markdown,
    Xml,
}

impl PackStyle {
    /// Строгий разбор стиля: принимает `"compact"`, `"markdown"`, `"xml"` (без учёта регистра).
    pub fn try_parse(style: &str) -> Result<Self> { /* bail на неизвестное значение */ }

    /// Совместимый обёрточный парсер для внутренних вызовов со старой сигнатурой.
    pub fn parse(style: &str) -> Self { Self::try_parse(style).unwrap_or(Self::Xml) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PackMode {
    Full,
    #[default]
    Compressed,
    Toc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompressOptions {
    pub strip_imports: bool,
    pub public_only: bool,
    pub exclude_tests: bool,
    pub strip_comments: bool,
    pub inline_elide: bool,
}

impl CompressOptions {
    pub fn legacy() -> Self {
        Self {
            strip_imports: false,
            public_only: false,
            exclude_tests: false,
            strip_comments: false,
            inline_elide: false,
        }
    }

    pub fn compact() -> Self {
        Self {
            strip_imports: true,
            public_only: false,
            exclude_tests: true,
            strip_comments: true,
            inline_elide: true,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PackRequest<'a> {
    pub subsystem_id: &'a str,
    pub mode: PackMode,
    pub style: PackStyle,
    pub compress_opts: CompressOptions,
    pub files: &'a [String],
    pub symbols: &'a [String],
}
```

- **Формат `PackStyle::Compact`:** заменяет XML/CDATA-теги на однострочный заголовок файла без декоративных баннеров:
  ```text
  === crates/ci-core/src/analysis/pack.rs (829L) ===
  // [3 use omitted]
  pub enum PackStyle { Compact, Markdown, Xml }
  pub fn pack_subsystem(...) -> Result<PackedSubsystems> { ⋮ }
  ```
- **Исправление XML-экранирования (`PackStyle::Xml`):** в `pack_files` экранировать атрибуты (`&`, `"`, `<`, `>`) и заменять `]]>` на `]]]]><![CDATA[>` внутри `<![CDATA[...]]>`.
- **Обратная совместимость:** функция `compress_source(content, ext)` продолжает делегировать в `adapter.compress_source(content)` (legacy-поведение для старых тестов `pack.rs:507, 537`), а новый код вызывает `compress_source_with(content, ext, opts)`. Смена дефолтного стиля на `"compact"` в `server.rs:474`, `src/cli/pack.rs:37` и `McpPackParams` сопровождается осознанным обновлением `src/cli/mod.rs:289`, `tests/cli.rs:237` и `tests/regression_benchmark.rs` (передающих `--style xml` там, где проверяется XML).

#### 1.2. Безопасная точечная фильтрация `files` и `symbols` (`pack.rs`, `tools.rs`, `handlers.rs`, `src/cli/pack.rs`)

1. **Валидация входных паттернов `files` до компиляции `globset::GlobSet`:**
   - Отклонять (возвращая ошибку уровня `InvalidParams` в MCP и код выхода `!= 0` в CLI) паттерны: пустые, содержащие `\0`, начинающиеся с `/` или `\`, имеющие Windows drive-префикс (`^[A-Za-z]:`), либо содержащие сегмент `..` (включая разделитель `\`).
   - Невалидный синтаксис глоба (`globset::Glob::new`) немедленно маппится в `rmcp::ErrorData::invalid_params`.
2. **Порядок сопоставления и security-инвариант (Zero-Bypass):**
   - Кандидаты формируются строго из `L0Index`: `sub.files ∪ sub.key_files` для конкретной подсистемы, либо объединение всех подсистем при `subsystem_id == "all"` (только при непустом `files`; голый `pack all` без фильтра по-прежнему ограничен `PACK_ALL_MAX_PAGES`).
   - Если в `files` передано короткое имя без `/` (например, `"pack.rs"`), допускается суффиксное совпадение `/{name}` или относительно директории подсистемы.
   - При непустом `files` и `0` совпадений возвращается явная ошибка со списком доступных файлов подсистемы (вместо тихого пустого результата).
   - **Symlink & Root Canonicalization:** перед чтением в `pack_subsystem` **и** `pack_files` каждый файл обязан пройти:
     - `!is_secret_path(path)`
     - `safe_join(root, path)`
     - `!fs::symlink_metadata(&full_p)?.file_type().is_symlink()`
     - `fs::canonicalize(&full_p)?.starts_with(&canonical_root)`
   - **Точечное расширение `is_secret_path` (`pack.rs:124-136`):** добавить `*.p12`, `*.pfx`, `*.jks`, `*.keystore`, `id_rsa*`, `id_ed25519*`, `.npmrc`, `.netrc`, `secrets.*`, `credentials.json`, `google-services.json` (без широких масок `*secret*`/`*credential*`).
3. **Проверка `PACK_ALL_MAX_PAGES` после фильтрации (`handlers.rs:570`):** для `subsystem_id == "all"` при непустом `files` оценка размера выполняется **после** фильтрации кандидатов.

#### 1.3. Адаптивная пагинация и защита курсора (`crates/ci-mcp/src/pagination.rs`)

- **Расширение `CursorData`:**
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
  pub struct CursorData {
      pub byte_offset: usize,
      #[serde(default)]
      pub page: usize,
      /// FNV-1a/SHA-256 64-bit префикс хеша полного payload и параметров запроса.
      #[serde(default, skip_serializing_if = "Option::is_none")]
      pub query_hash: Option<u64>,
  }
  ```
  Если передан курсор с `query_hash == Some(h)`, не совпадающим с текущим payload/запросом, возвращается явная ошибка/сообщение о недействительном курсоре (старые курсоры без `query_hash` сохраняют совместимость через `Option`).
- **Адаптивный `format_footer_ex`:**
  - Существующая `format_footer(truncated, next_cursor, stale, tokens)` сохраняет точный формат для `budget < 256` (включая `budget = 60` и `MIN_RESPONSE_TOKENS = 64`), гарантируя прохождение всех текущих тестов в `pagination.rs` и `ci-mcp/src/lib.rs`.
  - Новый `format_footer_ex(ctx: &FooterContext)` при `effective_max_tokens >= 256` добавляет `page: {page}/{total_pages}`, компактный список оставшихся файлов за `byte_offset` (например, `remaining: file1.rs, file2.rs (+3)`) и краткую директиву следующего шага (`next: pass cursor="..." or narrow with files=[...]`).
- **Окно $O(1)$ пагинации с безопасностью UTF-8:**
  - Вместо `10–30` полных кодирований `count_tokens(&rendered)` в `find_split_point` оценивать верхний байтовый срез `window_bytes = (target_body_tokens * 6).max(512)` от `remaining` (расширяя окно в 2x только если в нём меньше `target_body_tokens`), находить валидную UTF-8 границу `find_valid_char_boundary`, откатываться к ближайшему `\n` и подтверждать финальный `rendered` одним-двумя вызовами `count_tokens(&rendered) <= effective_max_tokens`. **Никогда** не вызывать `bpe.decode(&tokens[..k])` напрямую на срезе токенов во избежание обрыва многобайтового UTF-8 (кириллица, BSL, эмодзи).

---

### Волна 2: Языковые адаптеры, L1.5 TOC, бюджет CLI и синхронизация инструкций

#### 2.1. Расширение `LanguageAdapter` и исправление адаптеров (`crates/ci-core/src/analysis/lang/`)

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolSpan {
    pub kind: &'static str,   // "fn" | "struct" | "class" | "interface" | "trait" | "enum" | "mod"
    pub signature: String,    // Однострочная сигнатура <= 120 символов
    pub is_public: bool,
    pub line_start: usize,    // 1-based
    pub line_end: usize,      // 1-based
}

pub trait LanguageAdapter: Send + Sync {
    fn language_id(&self) -> &'static str;
    fn extensions(&self) -> &'static [&'static str];
    fn extract_symbols(&self, source: &str) -> Vec<String>;
    fn compress_source(&self, source: &str) -> String;
    fn method_bodies(&self, source: &str) -> Vec<(String, String)>;

    /// Расширенное сжатие с учётом CompressOptions и реального расширения файла.
    fn compress_with(&self, source: &str, _ext: &str, _opts: &CompressOptions) -> String {
        self.compress_source(source)
    }

    /// Извлечение диапазонов символов [line_start..=line_end] для L1.5 TOC и `symbols`.
    fn extract_symbol_spans(&self, _source: &str, _ext: &str) -> Vec<SymbolSpan> {
        Vec::new()
    }
}
```

- **Исправление `BraceLangAdapter` (`brace.rs:22`):** перенести распознавание по реальному `ext` (а не hardcoded `"ts"`!), чтобы Go (`func`, `type ... struct`), Java/C# (`public class`, `using`, `package`) и TS/JS (`import ... from`) корректно извлекали символы, сжимали импорты и находили тела методов.
- **Сжатие по языкам (`compress_with`):**
  - **Rust (`rust.rs` / `pack.rs`):** при `strip_imports` схлопывать `Item::Use` в `// [N use omitted]`; при `exclude_tests` фильтровать `#[cfg(test)]` через существующий `has_cfg_test` (`detector.rs:2039`); при `public_only` фильтровать через `is_syn_pub` (`detector.rs:2062`); при `strip_comments` убирать `///` / `//!` doc-атрибуты (либо обрезать до 1 строки); при `inline_elide` выводить `{ ⋮ }` в одну строку.
  - **Kotlin (`kotlin.rs`) и BraceLang (`brace.rs`):** учёт `package` / `import` / `using` преамбул; корректный трекинг строковых литералов, `"${...}"`, raw-строк `"""..."""`, блочных комментариев и expression-body функций `fun foo(...) = expr` (без `{}`!) при расчёте границ символов и тел `method_bodies`.
  - **Python (`python.rs`):** схлопывание `import`/`from ... import`; при `strip_comments` пропуск многострочных docstrings и `#`-комментариев; при `public_only` пропуск `_private` (кроме `__init__` и `__all__`-публичных).
  - **BSL (`bsl.rs`):** при `public_only` оставлять только процедуры/функции с `Экспорт` / `Export` и директивы `&НаСервере` / `&НаКлиенте`.
- **Выборка `symbols`:** при непустом `PackRequest::symbols` вырезать только совпадающие тела методов/классов по `extract_symbol_spans` / `method_bodies` без вывода остальных деклараций файла.

#### 2.2. Необрезаемый индекс уровня L1.5 (`PackMode::Toc` / `--toc` / `--signatures-only`)

- Формат вывода (группировка по файлам, 1 строка на символ или компактная строка на класс/методы, с точными `[Lstart-Lend]`):
  ```text
  [TOC] crates_ci-core_analysis_pack (1 file, 829L | ~210 tok)
  pack.rs (829L):
    enum PackStyle [L11-15] | struct PackedSubsystems [L30-37]
    fn pack_subsystem(root, knowledge_dir, index, id, compress, style) -> Result<PackedSubsystems> [L42-121]
  ```
- **Fallback для не-кодовых файлов:** для `.md` (заголовки `#`/`##` с `[Lstart]`), `.toml`/`.yaml`/`.json` (ключевые секции верхнего уровня с `[Lstart]`), `.sql`/`.sh` или прочих файлов (если `extract_symbol_spans` пуст) выводить компактный список `path (NL)` с ключевыми якорями, чтобы TOC не возвращал пустоту на подсистемах вроде `docs` и `scripts`.
- **Rust `SymbolSpan` без `syn/span-locations`:** строить однопроходную таблицу байтовых смещений начала строк + лексический сканер top-level и `impl` деклараций (или сопоставление с `syn` без `proc-macro2/span-locations`), сохраняя быструю сборку workspace.

#### 2.3. Бюджет вывода в CLI (`src/cli/pack.rs`)

- Добавить в `PackArgs`:
  - `--files <GLOB_OR_PATH>` (repeatable) и алиас `--filter <PATTERN>`
  - `--symbols <NAME>` (repeatable)
  - `--mode <full|compressed|toc>`, флаги `--toc` / `--signatures-only`
  - `--public-only`, `--keep-imports`, `--include-tests`
  - `--max-lines <usize>`, `--max-tokens <usize>`, `--cursor <String>`
- **Лестница авто-деградации (только для `stdout` при заданном бюджете `--max-lines` или `--max-tokens`, без затрагивания `-o <file>`):**
  1. **Уровень 0:** запрошенный режим. Если `lines <= max_lines` и `tokens <= max_tokens` — вывести как есть.
  2. **Уровень 1:** переключение на `PackStyle::Compact` + `CompressOptions::compact()` (с префиксом `[auto-compressed: exceeded budget -> compact]`).
  3. **Уровень 2:** если всё ещё не помещается — переключение в `PackMode::Toc` (если не был явно запрошен `--files` с `--no-compress` для чтения тел!) с подсказкой `[auto-toc: drill down via --files <f> --no-compress]`.
  4. **Уровень 3:** честная постраничная пагинация (через уже подключённый в root [`Cargo.toml:64`](file:///home/gidragir/projects/context-init/Cargo.toml#L64) модуль `ci_mcp::pagination`) с жёстким резервированием строк/токенов под закрывающий ```` ``` ```` (если страница оборвалась внутри code fence) и последнюю строку `--cursor "cur_..."`, чтобы итог **строго** не превышал `--max-lines` и `--max-tokens`.

#### 2.4. MCP `ServerConfig.instructions`, `rule_template.md`, `AGENTS.md` и `SKILL.md`

- В `ContextInitServer::get_info` ([`server.rs:695-703`](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/server.rs#L695-L703)) заполнить поле `instructions` кратким протоколом: обязательность `"path": "<repo_root>"`, маршрут `context_get_subsystems` → `context_pack(mode="toc")` → `context_pack(files=[...], compress=false)`, и реакция на `truncated: true` (`cursor` или сужение `files`).
- Синхронизировать `src/cli/setup/rule_template.md` (строго `<= 25` строк с проверкой `src/cli/setup/rules.rs:74-96`), `render_root_agents_md` (`generator.rs:1058-1065`), `skill_file` (`rules.rs:29-35`) и `.agents/skills/context-init/SKILL.md`:
  - Внедрить компактные JSON-шаблоны вызовов для снижения числа лишних обращений к lazy MCP-схемам.
  - Заменить устаревший путь `.agents/knowledge/**` в `SKILL.md:20` на `.context-init/knowledge/**` и убрать антипаттерн `-o /tmp/context.xml`.
  - Сохранить все проверяемые тестами маркеры (`context_get_subsystems`, `context_get_subsystem`, `context_pack`, `context_check`, `"Subagents and scripts"`, `"context-init pack <subsystem_id> --compress"`, `"{rtk}context-init search"`).

---

### Волна 3: Ускорение синхронизации (`git`, `scan`, `detector`, `handlers`, `pipeline`)

#### 3.1. Батчинг Git без поломки API (`crates/ci-core/src/git.rs` и `generator.rs`)

- Расширить `RepoGitSnapshot` ([`git.rs:35-40`](file:///home/gidragir/projects/context-init/crates/ci-core/src/git.rs#L35-L40)) **аддитивно** (сохранив `head_commit: String` и `status_entries: Vec<GitStatusEntry>`):
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
  pub struct RepoGitSnapshot {
      pub head_commit: String,
      pub status_entries: Vec<GitStatusEntry>,
      /// Карта `rel_path -> (object_type, sha)` из 1x `git ls-tree -r -t -z HEAD`.
      #[serde(default, skip)]
      #[schemars(skip)]
      pub tree_entries: BTreeMap<String, GitTreeEntry>,
      /// SHA корневого дерева `HEAD^{tree}` (для подсистем с directory == ".").
      #[serde(default, skip)]
      #[schemars(skip)]
      pub root_tree_sha: Option<String>,
      /// Признак нового репозитория без коммитов (unborn HEAD) или ошибки `ls-tree`.
      #[serde(default, skip)]
      #[schemars(skip)]
      pub unborn_head: bool,
  }
  ```
- **Обеспечение полной эквивалентности `SystemGitBackend::blob_sha` ([`git.rs:414-454`](file:///home/gidragir/projects/context-init/crates/ci-core/src/git.rs#L414-L454)):**
  1. **Unborn `HEAD`:** если `head_commit` пуст / `HEAD` не разрешён (`unborn_head = true`), не запускать `ls-tree`, а возвращать `Ok(None)` для директорий и `git hash-object` только для отслеживаемых/рабочих одиночных файлов (или fallback).
  2. **Корень `"."`:** обрабатывается из `root_tree_sha` (`git rev-parse --verify HEAD^{tree}` — 1 вызов при построении снимка).
  3. **Одиночный файл (single-file подсистема):** если путь помечен в `status_entries` (изменён в рабочем дереве или untracked) либо `unborn_head == true`, вызывать `git hash-object -- <norm_rel>` по рабочему дереву (сохраняя точный паритет с текущим `blob_sha`); для чистых отслеживаемых файлов брать `blob_sha` из `tree_entries` за $O(\log N)$ без процесса!
  4. **Подмодули (`160000 commit`):** распознавать тип `"commit"` при парсинге вывода `git ls-tree -r -t -z HEAD` (строгий разбор формата `<mode> <type> <sha>\t<path>\0`, сплит строго по `\t`), сохраняя поведение `rev-parse` / пропуска не-blob/tree без сбоев.
  5. **`paths_tree_sha` (`git.rs:105-128`):** для `paths.len() == 1` — возвращать raw SHA без SHA-256; для `>= 2` — вычислять `Sha256` от отсортированных `"{p}:{sha}\n"` полностью в памяти с использованием `snapshot.blob_sha_cached(repo_root, p)`.
- **Проброс `&RepoGitSnapshot` и `CardCache` в `generator::update_with_options` и `audit`:**
  - Чтение каждой L1-карточки в `generator::update_with_options` (`card_is_pristine` + `card_content`) и `run_check` (`audit` + `check_subsystem_staleness`) выполняется **1 раз** через локальный `CardCache = BTreeMap<String, CachedCard>`.
  - Переиспользование `snapshot` и группировка `diff_name_only` по уникальным `prev_commit` внутри `audit`/`update_with_options` устраняют $O(N)$ вызовов `git status`, `git rev-parse` и `git diff`.

#### 3.2. Единый проход по ФС и устранение повторного I/O (`scope.rs`, `scan.rs`, `detector.rs`, `subsystems.rs`, `handlers.rs`)

1. **`scan.rs`:** объединить `ScopeFilter::build_diagnostic` и `ScopeFilter::build` в один проход `ScopeFilter::build_with_diagnostic`; убрать отдельный `File::open` в `is_likely_binary` — читать заголовок 8 КБ из уже открытого `File` перед дочиткой до `TOKENIZE_MAX_BYTES`.
2. **`detector.rs`:**
   - Полностью удалить `thread_local! static SCOPE` ([`detector.rs:88-114`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/detector.rs#L88-L114)) и передавать `&ScopeFilter` явно во все функции `discover_subsystems` / анализа.
   - Перевести `discover_subsystems` на работу с уже собранным срезом `scope.files()` вместо повторных рекурсивных `fs::read_dir` / `contains_relevant_files_depth` / `collect_files_recursive_excluding`.
   - Заменить линейный `ancestors().find_map(...)` и $O(K^2)$ `symbols.contains(&s)` на индекс по подсистемам и `BTreeSet<String>`.
3. **Единый `AnalyzedFileCache` в памяти + внешний дисковый кэш (`StoragePaths::cache_dir`):**
   - Объединить два раздельных `FileMemo` (`detector.rs:1264` и `1802`), `count_lines` и `estimate_tokens_fast` в единый per-run `AnalyzedFileCache`: файл читается 1 раз, из `&str` сразу вычисляются `lines`, `symbols` и (при необходимости) `bpe_tokens`.
   - Опциональный дисковый кэш `<cache_dir>/<project_hash>/metrics_v1.json` хранит записи `(len, modified_nanos, sha256_hex, ci_core_version, lines, bpe_tokens, symbols)`. При совпадении `(len, modified_nanos)` на ФС с грубой гранулярностью `mtime` (1 сек) для файлов, изменённых недавно, сверять `sha2::Sha256`.
   - В `subsystems::build_deterministic` ([`subsystems.rs:96-98`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/subsystems.rs#L96-L98)) передавать вычисленную в `ProjectAnalysis` карту токенов вместо повторного обхода и BPE-токенизации всех файлов.
4. **`handlers.rs` (`get_subsystems` и `pack`):**
   - В `get_subsystems` (`L414`) при активной телеметрии суммировать уже посчитанные `summaries[i].tokens` вместо повторного вызова `live_source_tokens`.
   - Для неизменённых (non-stale) подсистем при наличии валидного кэша метрик в `cache_dir` брать `tokens` из кэша без чтения исходников с диска.

#### 3.3. Точечный `refresh_stale_card` (`pipeline.rs:209`)

- При `auto_refresh=true` на одиночной подсистеме (`handlers::get_subsystem`) выполнять локальный пересчёт метаданных/символов/SHA только для файлов этой подсистемы (обновляя одну L1-карточку и соответствующую запись в `L0_index.json`), **только если**:
  - подсистема не относится к `mdo` (1С-объекты собираются кросс-каталожно в `mdo::discover_mdo_subsystems`);
  - `sub.parent.is_none()` и размер подсистемы не пересекает `DEFAULT_SPLIT_THRESHOLD_TOKENS`;
  - не изменилась структура подкаталогов с появлением новых манифестов (`Cargo.toml`, `package.json`, `build.gradle.kts`).
- В остальных случаях автоматически выполнять стандартный безопасный fallback на `run_update`.

---

## 5. План верификации и критерии успеха

### 5.1. Автоматизированные тесты по волнам

| Волна | Обязательные тесты и проверки |
| :---: | :--- |
| **Волна 1** | • Unit-тесты `pack.rs`: сохранение legacy-тестов `compress_source`, новые тесты `PackStyle::Compact`, экранирование `]]>` и атрибутов в XML, фильтрация `files` (`globset`, короткие имена, ошибка при 0 совпадений), `symbols`, отклонение симлинков, `..`, `/`, `\`, `C:\`, новых паттернов `is_secret_path`.<br>• Тесты `pagination.rs` и `ci-mcp`: прохождение всех старых кейсов на `budget = 60..64`, расширенный футер при `budget >= 256`, отклонение устаревшего `CursorData.query_hash`, прохождение `tests/cli.rs` и `tests/regression_benchmark.rs`. |
| **Волна 2** | • Unit-тесты `lang/{rust,kotlin,brace,python,bsl}.rs`: `compress_with` (`strip_imports`, `public_only`, `exclude_tests`, `strip_comments`, `{ ⋮ }`), исправление `BraceLangAdapter` для Go/Java/C#/TS, точность `SymbolSpan` (`[Lstart-Lend]`) на Kotlin expression-body, `${...}`, `"""..."""` и fallback TOC на `.md`/`.toml`.<br>• Интеграционные тесты `src/cli/pack.rs`: `--max-lines 90`, `--max-tokens`, `--cursor`, закрытие code fence при переносе страницы.<br>• Тесты правил `src/cli/setup/rules.rs` (`<= 25` строк) и `tests/cli.rs` (`AGENTS.md <= 22 000` байт). |
| **Волна 3** | • Unit-тесты `git.rs`: паритет `RepoGitSnapshot` (`ls-tree`) с `SystemGitBackend::blob_sha` на unborn `HEAD`, `"."`, dirty single-file (`hash-object`), подмодулях и `paths_tree_sha`.<br>• Тесты детерминизма `analysis/mod.rs`: `l0_omits_volatile_fields` строго зелёный, кэш находится только в `cache_dir` (вне repo), `refresh_stale_card` откатывается на полный update при `.mdo` и `parent.is_some()`.<br>• UTF-8 тест пагинации `pagination.rs`: кириллица (BSL/русские комментарии) и 4-байтовые эмодзи на границе страницы без паник и ошибок декодирования. |

### 5.2. Регрессионный контроль и замеры производительности

После каждой волны прогоняется полный набор проверок и фиксация замеров в `reports/benchmark_latest.json`:
1. `rtk cargo fmt --all -- --check`
2. `rtk cargo clippy --workspace --all-targets -- -D warnings`
3. `rtk cargo test --workspace`
4. Замер экономии токенов на тестовых фикстурах (`context-init` + Kotlin/Gradle фикстура) для `PackStyle::Xml`, `Markdown`, `Compact` и `PackMode::Toc`, а также замер времени `scan`, `check` и инкрементального `update` (сравнение до/после Волны 3).
