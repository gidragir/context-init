# Независимое техническое ревью: отчёт об улучшениях `context-init`

> **Объект проверки:** [`2026-11-15-context-init-improvements-research.md`](file:///home/gidragir/projects/context-init/reports/subagents/2026-11-15-context-init-improvements-research.md)  
> **Кодовая база:** `/home/gidragir/projects/context-init` (`crates/ci-core`, `crates/ci-mcp`, `src/cli`, `tests`, `Cargo.toml`)  
> **Примечание:** исходный код, тесты и конфигурации в рамках ревью не изменялись.

---

## 1. Резюме верификации и вердикт

**Вердикт: Условно принято — к реализации допускается только после корректировки контрактов и исправления 7 блокеров.**

Исследовательская диагностика узких мест (`PackStyle::Xml` по умолчанию, неинформативный футер, отсутствие `files`/`symbols`/`toc`, повторное чтение файлов и N+1 `git`-подпроцессы) **в целом подтверждается исходным кодом**. Однако прямой перенос предложенных API-решений без корректировки приведёт к:
1. Падению существующих unit-, integration- и benchmark-тестов (`PackStyle::Xml`, `compress_source`, `format_footer`, `budget = 60/64`, `l0_omits_volatile_fields`).
2. Дефектам корректности и безопасности: тихому обходу `[scope]` при параллелизме `rayon` из-за `thread_local! SCOPE`, сбою `bpe.decode` на многобайтовом UTF-8 (кириллица/BSL), потере семантики single-file/unborn-HEAD в `git ls-tree`, symlink-escape при чтении `sub.files` без `canonicalize`, ложноположительным блокировкам исходников при `*secret*`.
3. Поломке инкрементальной консистентности 1С-проектов (`.mdo`) и split-крейтов при наивном `refresh_stale_card` по одной директории.

---

## 2. Подтверждённые находки

| Область | Что проверено в коде | Статус |
| :--- | :--- | :--- |
| Дефолтный формат упаковки | `PackStyle::Xml` (`#[default]`, [pack.rs:11-26](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/pack.rs#L11-L26)), `"xml"` в [server.rs:474](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/server.rs#L474) и [src/cli/pack.rs:37](file:///home/gidragir/projects/context-init/src/cli/pack.rs#L37); `DEFAULT_MAX_RESPONSE_TOKENS = 4000` ([config.rs:17](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L17)) | Подтверждено |
| Бенчмарки | `detector.rs`: `18704 -> 3275` токенов (`11193` байт), `drift_metrics.avg_ms = 15.10`, `scan_files = 400` за `20.33 ms`, `stale_check_avg_ms = 7.70` ([benchmark_latest.json:3-18,60-63](file:///home/gidragir/projects/context-init/reports/benchmark_latest.json#L3-L18)) | Подтверждено |
| Размеры файлов и доля тестов | `generator.rs`: 2220 строк (`#[cfg(test)]` на L1142, 48.6%); `pack.rs`: 829 (L476, 42.7%); `detector.rs`: 2509 (`mod tests;` на L2508); `pagination.rs`: 542 (L373, 31.4%); `kotlin.rs`: 511 (L408, 20.4%) | Подтверждено |
| Адаптеры языков | `BraceLangAdapter::extract_symbols` хардкодит `"ts"` ([brace.rs:21-23](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/lang/brace.rs#L21-L23)); `method_bodies` пуст в `brace.rs:143-145` и `kotlin.rs:210-212`; Rust-пакер оставляет `use`, `#[cfg(test)]` и 3-строчный `⋮----` | Подтверждено |
| Параметры фильтрации / TOC | `McpPackParams` ([tools.rs:90-108](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/tools.rs#L90-L108)) и `PackArgs` ([src/cli/pack.rs:18-39](file:///home/gidragir/projects/context-init/src/cli/pack.rs#L18-L39)) не имеют `files`, `symbols`, `mode`, `--max-lines`, `--max-tokens`, `--cursor` | Подтверждено |
| Пагинация и футер | `CursorData` хранит только `byte_offset`, `page` ([pagination.rs:10-16](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/pagination.rs#L10-L16)); `find_split_point` + 2x `count_tokens(&rendered)` в цикле ([pagination.rs:255-293](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/pagination.rs#L255-L293)) | Подтверждено |
| MCP-инструкции и шаблоны | `get_info` ([server.rs:695-703](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/server.rs#L695-L703)) не задаёт `instructions`; [SKILL.md:40,91,94](file:///home/gidragir/projects/context-init/.agents/skills/context-init/SKILL.md#L40) содержит `-o /tmp/context.xml`, а строка `20` ещё и устаревший путь `.agents/knowledge/**` | Подтверждено |
| Двойной обход и I/O в `scan` и `detector` | Два `WalkBuilder` в [scan.rs:146,241](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/scan.rs#L146) + отдельный `File::open` в `is_likely_binary` ([scan.rs:30,164](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/scan.rs#L30)); `scoped_files` в `detector.rs:164,188` используется только для `mdo`, остальное заново обходит `fs::read_dir` (`L822,1155,1637`); два отдельных `FileMemo` (`L1264,1802`), `ancestors().find_map` (`L1771`), $O(K^2)$ `symbols.contains` (`L1785`) | Подтверждено |
| Повторный BPE и отбрасывание метрик | `#[serde(skip)]` на `SubsystemFile::lines` и `SubsystemInfo::tokens` ([storage.rs:254,295](file:///home/gidragir/projects/context-init/crates/ci-core/src/storage.rs#L254)); `subsystems::build_deterministic` пересчитывает `subsystem_tokens` ([subsystems.rs:96-98](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/subsystems.rs#L96-L98)); `handlers::get_subsystems` считает токены дважды при телеметрии ([handlers.rs:376,414](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/handlers.rs#L376)); `handlers::pack` вызывает `live_source_tokens` (`L566`) | Подтверждено |
| N+1 Git и повторное чтение карточек | `run_check` дважды загружает `L0_index.json` и дважды читает L1-карточки ([pipeline.rs:141,151,158](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/pipeline.rs#L141)); `update_with_options` читает карточки в `card_is_pristine` (`L614`) и `card_content` (`L157`) и создаёт `SystemGitBackend` на каждую карточку в `git_source_shas` (`L76`) и `is_git_stale` (`L244`) | Подтверждено |
| Грубый `refresh_stale_card` | Вызывает полный `detect(project, false)` + `build_deterministic` + `generator::update` ([pipeline.rs:209-217](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/pipeline.rs#L209-L217)), а `get_subsystem` повторно проверяет staleness ([handlers.rs:472](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/handlers.rs#L472)) | Подтверждено |
| Размер `AGENTS.md` | Текущий размер `AGENTS.md` — `6014` байт (лимит `<= 22 000` байт соблюдён с большим запасом) | Подтверждено |

---

## 3. Критические замечания, ошибки и блокеры

### 3.1. Фактические неточности в отчёте ресерчера

1. **Заявлено «22 строки ≤ 25» для `rule_template.md` ([research.md:310](file:///home/gidragir/projects/context-init/reports/subagents/2026-11-15-context-init-improvements-research.md#L310)):** приведённый в отчёте шаблон (`L312-324`) содержит **13 строк**, а не 22 (тест `rules.rs:74-76` пройдёт, но счётчик в отчёте неверен). Более серьёзно: шаблон рекламирует `"style": "compact"`, `"mode": "toc"`, `"files"`, `--style compact --max-lines 90` до реализации этих флагов — если применить его в Волне 1 без кода Волны 2, CLI упадёт с ошибкой парсинга `clap`, а MCP молча откатится в XML через `PackStyle::parse`.
2. **Утверждение про «однопоточную токенизацию» (`research.md:20, 388`):** `detector::tokens_if_over` ([detector.rs:1241-1253](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/detector.rs#L1241-L1253)) **уже** распараллеливает `tokenize_files` через `std::thread::scope`. Однопоточными остаются `scan_project_with`, `build_deterministic` и `handlers::get_subsystems`.
3. **Зависимости `Cargo.toml` отсутствуют (`research.md:247, 419, 433`):** в [Cargo.toml](file:///home/gidragir/projects/context-init/Cargo.toml#L18-L43) и [crates/ci-core/Cargo.toml](file:///home/gidragir/projects/context-init/crates/ci-core/Cargo.toml#L15-L31) **нет** `rayon`, **нет** `blake3` (есть только `sha2 = "0.10"`) и **нет** feature `"span-locations"` у `syn = { version = "2.0", features = ["full", "parsing"] }`. Предложение ресерчера (`research.md:247`) «использовать `Spanned::span()` без `"span-locations"`» не работает: без `proc-macro2/span-locations` координаты строк недоступны.
4. **Двойной `render_l0_json(&index)` в `generator.rs:695, 708` (`research.md:377`):** между этими двумя вызовами при `changed == true` строки `L700-701` мутируют `index.updated_at` и `index.generator`. Просто закэшировать JSON с `L695` нельзя (потеряется обновление метаданных).
5. **Перенос `pagination.rs` в `ci-core` (`research.md:265`):** в root [Cargo.toml:64](file:///home/gidragir/projects/context-init/Cargo.toml#L64) уже есть прямая зависимость `ci-mcp.workspace = true`, поэтому `src/cli/pack.rs` может использовать `ci_mcp::pagination` сразу без срочного переноса модуля.
6. **Неверифицируемые ссылки:** в репозитории отсутствует `scripts/check-hygiene.sh` (в `scripts/` есть `audit-sessions.py`, `probe-adoption.sh`, `probes/`, `tests/`); метрика `cursor_calls = 0` отсутствует в кодовой базе и отчётах (`reports/`), т.е. не подтверждается артефактами репозитория.

### 3.2. Блокеры совместимости контрактов и тестов

1. **Смена дефолта `PackStyle::Xml` -> `Compact` и модификация `compress_source` ломают минимум 4 группы тестов:**
   - [src/cli/mod.rs:289](file:///home/gidragir/projects/context-init/src/cli/mod.rs#L289): `assert_eq!(a.style, "xml");`.
   - [tests/cli.rs:237-251](file:///home/gidragir/projects/context-init/tests/cli.rs#L237-L251): `cli_pack_compress_outputs_xml` проверяет `stdout(&out).trim_start().starts_with('<')`.
   - [crates/ci-core/src/analysis/pack.rs:507-509, 537-546](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/pack.rs#L507-L546): тесты `compress_source(code, "rs")` жёстко требуют `{\n    ⋮----\n}`, `#![allow(unused)]` и `#[cfg(test)]\nmod tests {\n    pub fn internal() {\n        ⋮----\n    }\n}`.
   - [crates/ci-core/src/analysis/lang/mod.rs:19, 110, 155, 169, 187, 201](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/lang/mod.rs#L19), [kotlin.rs:453](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/lang/kotlin.rs#L453), [tests/regression_benchmark.rs:184, 291, 579](file:///home/gidragir/projects/context-init/tests/regression_benchmark.rs#L184): завязаны на `ELIDED_BODY = "⋮----"`, сигнатуру `format_rust_items` и дефолт `style: None` в `server.pack`.
   - Кроме того, `PackStyle::parse` ([pack.rs:19-25](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/pack.rs#L19-L25)) молча превращает любую неизвестную строку в `Xml` — при переключении дефолта на `Compact` опечатка в `--style` или незнакомый параметр тихо сменит формат вместо ошибки валидации.

2. **Расширенный `format_footer` нарушает бюджет `60–64` токена и ломает контракт парсинга:**
   - [crates/ci-mcp/src/lib.rs:108-127, 162-164, 2059, 2111-2119](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/lib.rs#L108-L127) и [pagination.rs:414-426, 463-465, 484-515](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/pagination.rs#L414-L426) жёстко проверяют сигнатуру `format_footer(truncated, next_cursor, stale, tokens)` и префикс `"[truncated: true | next_cursor: \"cur_"`, а также пагинацию при `budget = 60` и `MIN_RESPONSE_TOKENS = 64` ([config.rs:23](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L23)).
   - Предложенный в [research.md:303](file:///home/gidragir/projects/context-init/reports/subagents/2026-11-15-context-init-improvements-research.md#L303) футер (`page: 1/3 ... | remaining: ... | next: call context_pack(...) ...`) вместе с `cur_...` весит ~65–85+ токенов — при `budget = 60..64` и `footer_reserve = 35` ([pagination.rs:250](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/pagination.rs#L250)) **не помещается в бюджет даже при пустом теле**, что гарантированно сломает тесты `out.token_count <= budget`.

3. **Снятие `#[serde(skip)]` с `lines` и `tokens` в `storage.rs` напрямую ломает тест и порождает шум в Git:**
   - Тест `l0_omits_volatile_fields` ([crates/ci-core/src/analysis/mod.rs:109-139](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/mod.rs#L109-L139)) **прямо запрещает** наличие `"\"root_path\""`, `"\"lines\""`, `"\"tokens\""` в `L0_index.json` и слов `"Строк"`, `"токенов"`, `"Токены"` в Markdown-артефактах.
   - Запись `tokens`/`lines` в коммиченный `L0_index.json` смешивает декларативную карту с волатильными метриками: при правке комментария в файле меняются `lines`/`tokens`, загрязняя diff, а при незакоммиченных локальных правках сохранённые `sub.tokens` в `L0_index.json` всё равно недействительны.

4. **Breaking change в `RepoGitSnapshot` ([git.rs:35-40](file:///home/gidragir/projects/context-init/crates/ci-core/src/git.rs#L35-L40)) и дефекты `git ls-tree -r -t -z HEAD`:**
   - Переименование `head_commit -> head: Option<String>` и `status_entries -> dirty_paths` ломает публичный API `ci-core`.
   - `git ls-tree -r -t -z HEAD` **не эквивалентен** текущему `SystemGitBackend::blob_sha` ([git.rs:414-454](file:///home/gidragir/projects/context-init/crates/ci-core/src/git.rs#L414-L454)) в 5 случаях:
     1. **Unborn `HEAD` (новый репо с 0 коммитами):** `ls-tree HEAD` и `rev-parse HEAD` падают.
     2. **Корень `"."` отсутствует в `ls-tree`:** для multi-dir подсистем `subsystem_directory` ([generator.rs:69](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/generator.rs#L69)) и `paths_tree_sha` ([git.rs:108](file:///home/gidragir/projects/context-init/crates/ci-core/src/git.rs#L108)) запрашивают `"."` (`HEAD^{tree}`), чего нет в выводе `ls-tree -r`.
     3. **Расхождение для single-file подсистем:** сейчас `blob_sha` для файла вызывает `git hash-object -- <norm_rel>` по **рабочему дереву** (учитывает untracked и dirty-правки), а `ls-tree` читает только `HEAD`.
     4. **Подмодули (`160000 commit`):** `ls-tree -t` возвращает gitlink без содержимого подмодуля.
     5. **`paths_tree_sha` ([git.rs:111-127](file:///home/gidragir/projects/context-init/crates/ci-core/src/git.rs#L111-L127)):** для 1 пути возвращает raw SHA, для `>= 2` — SHA-256 от `"{p}:{sha}\n"`; любое отклонение пометит все существующие карточки как `stale`.

### 3.3. Риски безопасности, многобайтового UTF-8 и параллелизма

1. **Гонка и обход `[scope]` при `rayon` в `detector.rs`:**
   - Сейчас область видимости в `detector.rs` хранится в `thread_local! static SCOPE` ([detector.rs:88-114, 165](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/detector.rs#L88-L114)).
   - Рабочие потоки пула `rayon` не наследуют `thread_local!` родительского потока: в воркерах `SCOPE` окажется `None`, и `scope_allows_file` / `scope_allows_dir` **молча вернут `true`**, полностью отключив `.gitignore` и `[scope]`.
2. **Ошибка в $O(1)$ BPE-срезе на многобайтовом UTF-8 (`pagination.rs`):**
   - Алгоритм в [research.md:440](file:///home/gidragir/projects/context-init/reports/subagents/2026-11-15-context-init-improvements-research.md#L440) (`encode(rest)` -> `decode(&tokens[..body_budget])`) **неверен для UTF-8**: в `cl100k_base` символы кириллицы (BSL, русские комментарии) и эмодзи (`🦀`) часто дробятся на 2–3 отдельных байтовых BPE-токена. Срез `&tokens[..k]` разрежет UTF-8 последовательность посередине, и `bpe.decode(...)` вернёт `Err`.
   - Кодировать весь `rest` целиком при огромном `pack all` (десятки тысяч токенов) ради 1 страницы в `4000` токенов расточительно — кодировать нужно ограниченное байтовое окно с расширением при необходимости, а границу искать через байтовый сдвиг `decode_bytes` + `find_valid_char_boundary` с последующим точным `count_tokens(&rendered)` (учитывая и отступ, и код-фенс в CLI).
3. **Отсутствие привязки `CursorData` к запросу и контенту:**
   - `CursorData` ([pagination.rs:10-16](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/pagination.rs#L10-L16)) содержит только `byte_offset` и `page`. При смене `files`/`mode`/`style` или правке файла между вызовами `cursor` продолжит резать новый текст по старому смещению.
4. **Пробелы безопасности в `files` / `pack.rs`:**
   - `safe_join` ([storage.rs:347-355](file:///home/gidragir/projects/context-init/crates/ci-core/src/storage.rs#L347-L355)) выполняет только лексическую проверку: на POSIX `"a\\..\\secret"` считается `Component::Normal`; `pack_subsystem` ([pack.rs:102](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/pack.rs#L102)) проверяет `.exists()` без отклонения симлинков, а `pack_files` ([pack.rs:156,182](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/pack.rs#L156)) вообще повторно делает `root.join(file_rel)` — симлинк, записанный в чужой `L0_index.json`, приведёт к чтению файла вне `root`.
   - `is_secret_path`: предложение добавить `*secret*` и `*credential*` ([research.md:205](file:///home/gidragir/projects/context-init/reports/subagents/2026-11-15-context-init-improvements-research.md#L205)) заблокирует легитимный код (`secret_rotation.rs`, `CredentialsProvider.kt`, `test_secrets.py`).
   - XML-генератор в `pack.rs:174-195` не экранирует `]]>` внутри `<![CDATA[...]]>` и атрибуты `id`/`name`/`path`.
   - На POSIX `Path::new("C:\\foo")` не распознаётся как `Component::Prefix`: проверка `files`-паттернов должна явно отклонять ведущие `/`, `\`, Windows drive-префиксы (`[A-Za-z]:`), NUL-байты, невалидные глобы (`rmcp::ErrorData::invalid_params`, [server.rs:239](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/server.rs#L239), вместо паники/500), а при `subsystem_id == "all"` + `files` фильтрация должна выполняться **до** чтения файлов и проверки `PACK_ALL_MAX_PAGES`.

---

## 4. Архитектурные корректировки и пропущенные краевые случаи

1. **1С `.mdo` и split-крейты ломают точечный `refresh_stale_card`:**
   - `mdo::discover_mdo_subsystems` ([mdo.rs:271-360](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/lang/mdo.rs#L271-L360)) объединяет в одну подсистему `.mdo` и `.bsl` из разных каталогов (`Catalogs/...`, `Documents/...`), а `subsystem_directory` ([generator.rs:42-73](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/generator.rs#L42-L73)) для разнородных путей возвращает `"."`. Точечное пересканирование одной директории сломает состав `sub.files`, а для split-крейтов Rust (`sub.parent.is_some()`) не увидит пересечение `DEFAULT_SPLIT_THRESHOLD_TOKENS` (`15_000`) на уровне родительского крейта.
2. **Персистентный кэш метрик (`FileMetricsEntry`) и место хранения:**
   - На ФС с 1-секундной гранулярностью `mtime` перезапись файла той же длины в пределах секунды пройдёт мимо `(len, modified_nanos)`. Нужно либо хэшировать содержимое через уже подключённый `sha2::Sha256` (без добавления `blake3`), либо проверять `ctime`/inode + hash при совпадении `(len, mtime)`, а в ключ кэша включать `ci_core::VERSION` и опции экстракции.
   - Хранить кэш следует строго в `storage_paths.cache_dir` ([config.rs:45](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L45)), а не в `.context-init/.cache/`: незаигноренный файл в рабочем дереве немедленно сделает `git status` грязным и пометит корневую подсистему как `stale`.
3. **Уязвимые места L1.5 TOC (`SymbolSpan`) без AST:**
   - В Kotlin / BraceLang построчный счёт `{}` ошибается на: expression-body функциях без скобок (`fun foo(x: Int) = x * 2`), `{}` в default-параметрах (`cb: () -> Unit = {}`), строковой интерполяции `"${...}"`, многострочных raw-строках `"""..."""` и комментариях.
   - `LanguageAdapter::compress_with` без передачи `ext` (или разделения `BraceLangAdapter`) не позволит по-разному сжимать `import` в Go (`import (...)`), TypeScript (`import ... from`) и Java/C# (`using`/`namespace`).
   - В режиме `PackMode::Toc` не учтены не-кодовые файлы (`.toml`, `.yaml`, `.md`, `.json`, `.sql`, `.sh`): для них нужен компактный fallback (заголовки `#` в Markdown / top-level ключи или строка `file (NL)`), иначе в `docs`/`scripts` подсистемах TOC будет пустым.
4. **Асимметрия `cli/pack` (`--max-lines` / закрытие code fence):**
   - Вставка закрывающего ```` ``` ```` или подсказки `--cursor` *после* нарезки нарушит лимит `--max-lines` / `--max-tokens`. Закрывающий fence и хвост обязаны резервироваться до расчёта сплита.
5. **Оценка `3x–10x` / `15x–25x` ускорения в таблице B.4 (`research.md:448-454`):**
   - Не выделена стоимость холодного старта `tiktoken_rs::cl100k_base()` ([tokens.rs:5,13](file:///home/gidragir/projects/context-init/crates/ci-core/src/tokens.rs#L5-L13)) и процессов `git`: на малом проекте (`15.1 ms` в `check`) загрузка BPE-словаря и форки `git` задают нижний порог латентности, а на крупных репо эффект `rayon` ограничивается дисковым I/O и кэшем страниц ОС — на холодном диске ускорение будет ниже заявленных верхних границ (`50x+` на `refresh_stale_card` достижимо лишь при отказе от полного `detect` на больших монорепо).

---

## 5. Рекомендации для финальной спецификации

### Принять (с уточнениями)
1. **Точечный выбор `files` + `symbols` + `mode="toc"` (A.2 + A.3):** внедрить в `PackRequest` / `McpPackParams` / `PackArgs`. Применять `files` **до** чтения/сжатия и **до** `PACK_ALL_MAX_PAGES`; проверять паттерны на `\`, ведущие `/`, drive prefix, `..`, валидировать `globset` с возвратом `rmcp::ErrorData::invalid_params`; проверять `canonical_path.starts_with(&canonical_root)` и `!fs::symlink_metadata(&p)?.file_type().is_symlink()`; в `is_secret_path` добавить точечные паттерны ключей/конфигов (`*.p12`, `*.pfx`, `*.jks`, `*.keystore`, `id_rsa*`, `id_ed25519*`, `.npmrc`, `.netrc`, `secrets.*`, `credentials.json`), но **не** `*secret*`/`*credential*` для кода.
2. **Единый проход по ФС и in-memory кэш (`B.2`):** объединить `ScopeFilter::build_diagnostic` + `build` в один проход; убрать второй `File::open` из `is_likely_binary`; передать `&ScopeFilter` явно во все функции `detector.rs` (удалив `thread_local! SCOPE`) и использовать `scope.files()` вместо повторных `fs::read_dir`; объединить два `FileMemo` (`L1264, L1802`) в единый `AnalyzedFileCache` на время запуска; исправить баг `"ts"` в `BraceLangAdapter::extract_symbols` (`brace.rs:22`).
3. **Батчинг Git (`B.1`) без поломки API:** добавить `tree_shas` и `root_tree_sha: Option<String>` к существующему `RepoGitSnapshot` с сохранением `head_commit` и `status_entries`; добавить `git ls-tree -r -t -z HEAD` + отдельный `HEAD^{tree}` (с fallback на unborn `HEAD` и проверкой `dirty_paths` для single-file `hash-object`), сплит строк строго по `\t`; передавать `&RepoGitSnapshot` и `CardCache` в `update_with_options`, `audit`, `card_content` и `is_git_stale`, сгруппировав `diff_name_only` по уникальным `prev_commit`.
4. **Чистая телеметрия и устранение двойного BPE в `handlers.rs`:** переиспользовать вычисленные `tokens` из `ProjectAnalysis` в `build_deterministic`, а в `get_subsystems` суммировать уже посчитанные `tokens` из `summaries` вместо повторного вызова `live_source_tokens` (`L414`).
5. **CLI `--max-lines`, `--max-tokens`, `--cursor` (A.4) и `ServerConfig.instructions` (`server.rs:695`):** реализовать лестницу авто-деградации (включая резерв под закрывающий fence и подсказку `--cursor`) через существующую зависимость на `ci_mcp::pagination`.

### Скорректировать перед внедрением
1. **Эволюционный переход формата и футера (A.1 + A.5):**
   - Оставить `compress_source(content, ext)` в legacy-режиме (`CompressOptions::legacy()`, `⋮----`) для обратной совместимости unit-тестов; ввести `compress_source_with(content, ext, &CompressOptions)` и отдельный вариант `PackStyle::Compact`.
   - Смену дефолта `style` (`"xml"` -> `"compact"`) вынести осознанным шагом с явным обновлением контракта в `src/cli/mod.rs:289`, `tests/cli.rs:237` и `tests/regression_benchmark.rs` (не молчаливый fallback в `PackStyle::parse`, а валидация допустимых значений).
   - **Компактный `format_footer`:** сохранить обратную совместимость для малых бюджетов (`60–64` токена!) или разделить режимы: компактный машинный маркер (`[truncated: true | page: 1/3 | next_cursor: "cur_..." | stale: false | tokens: 3950]`, укладывается в `<= 35` токенов), а `remaining_files` / verbose-подсказку добавлять **только** при `effective_max_tokens >= 256` (не ломая `budget = 60` и `MIN_RESPONSE_TOKENS = 64`).
   - В `CursorData` добавить `#[serde(default)] query_hash: u64` (хеш параметров/текста) для защиты от устаревших или несовпадающих курсоров.
2. **Обновление `rule_template.md`, `AGENTS.md`, `SKILL.md`:** синхронизировать строго **в одном PR/волне** с появлением флагов `--style compact`, `--toc`, `--files`, `--max-lines`; поправить устаревший путь `.agents/knowledge/**` в `SKILL.md:20` на `.context-init/knowledge/**`; проверить все assertions в `rules.rs:74-96` и `tests/cli.rs:754-768`.
3. **Ускорение пагинации без ошибок UTF-8:** вместо наивного `bpe.decode(tokens[..k])` кодировать ограниченное байтовое окно текста и откатываться к валидной UTF-8 + `\n` границе с финальной проверкой `count_tokens(&rendered) <= effective_max_tokens`.

### Отложить / Отклонить
1. **Отклонить запись `tokens`/`lines` в коммиченный `L0_index.json` (снятие `#[serde(skip)]`):** вместо ломки инварианта `l0_omits_volatile_fields` (`analysis/mod.rs:109`) вынести кэш файловых метрик (`len`, `mtime`, `sha256`, `tokens`, `lines`, `symbols`) строго в `storage_paths.cache_dir` ([config.rs:45](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L45), вне Git-дерева).
2. **Отклонить добавление `blake3`:** использовать уже подключённый `sha2::Sha256`.
3. **Отложить `rayon` в `detector` и агрессивный `refresh_stale_card`:** сначала убрать `thread_local! SCOPE` и промерить прирост на реальном монорепо; `refresh_stale_card` ограничить пересчётом только метаданных/SHA одной карточки, делая fallback на полный инкрементальный пересчёт при `.mdo` или `sub.parent.is_some()`.
4. **Отложить добавление `syn/span-locations` до замера времени сборки и альтернативы в виде 1-проходного Rust-сканера строк:** для `L1.5 TOC` лексический line-spanner может дать `[Lstart-Lend]` без глобального утяжеления `proc-macro2`.
