# context-init: Компактная упаковка контекста, L1.5 TOC, бюджет CLI и ускорение синхронизации — план реализации

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Внедрить компактный формат упаковки `PackStyle::Compact`, точечный выбор `files`/`symbols`, необрезаемый индекс L1.5 (`mode="toc"`/`--toc`), бюджет вывода CLI (`--max-lines`/`--max-tokens`/`--cursor`), адаптивный футер пагинации с защитой курсора, инструкции MCP и кратное ускорение синхронизации (`git ls-tree`, единый обход ФС, внешний кэш метрик, точечный `refresh_stale_card`).

**Architecture:** Разработка ведётся в 3 волны по крейтам `ci-core` → `ci-mcp` → CLI (`src/cli`). Существующие публичные контракты (`compress_source`, `pack_subsystem`, `pack_files`, `paginate_content`, поля `RepoGitSnapshot`, `L0_index.json` v4 без волатильных полей) сохраняются или расширяются аддитивно через `*_with` / новые структуры запросов, а шаблоны инструкций (`rule_template.md`, `AGENTS.md`, `SKILL.md`) обновляются строго синхронно с появлением соответствующих флагов.

**Tech Stack:** Rust 2021, `ci-core`, `ci-mcp`, `serde`, `serde_json`, `globset`, `ignore`, `sha2`, `tiktoken-rs`, `quick-xml`, `clap`, `rmcp`, `rtk`.

**Spec:** `docs/superpowers/specs/2026-11-15-context-init-pack-and-sync-improvements-design.md`

## Global Constraints

- Все терминальные команды выполняются строго с префиксом `rtk` (`rtk cargo ...`, `rtk git ...`); GNU `find` и `grep` запрещены (использовать `rtk fd` и `rtk rg` без `-E`).
- Для навигации по существующим модулям `> 1 000` токенов использовать MCP `context_pack(path="/home/gidragir/projects/context-init", style="markdown")` или `rtk context-init pack <subsystem_id> --compress`.
- Схема `L0_index.json` остаётся `CURRENT_L0_SCHEMA_VERSION = 4`; тест `l0_omits_volatile_fields` (`crates/ci-core/src/analysis/mod.rs:109`) остаётся незыблемым: `"tokens"`, `"lines"`, `"root_path"` не сериализуются в `L0_index.json`.
- Кэш файловых метрик хранится строго в `StoragePaths::cache_dir` (`crates/ci-core/src/config.rs:45`), никогда внутри рабочего дерева репозитория; для хеширования используется уже подключённый `sha2::Sha256` (без добавления `blake3`).
- Legacy-функция `compress_source(content, ext)` сохраняет поведение `CompressOptions::legacy()` (`⋮----`, сохранение `#[cfg(test)]`) для существующих unit-тестов; новое сжатие вызывается через `compress_source_with(..., &CompressOptions)`.
- Безопасность путей в `pack_subsystem` и `pack_files`: отклонение `/`, `\`, `[A-Za-z]:`, `..`, `\0`, невалидных глобов, симлинков (`!symlink_metadata()?.file_type().is_symlink()`) и путей за пределами `canonicalize(root)`.
- Бюджет футера пагинации при `effective_max_tokens < 256` (включая `budget = 60` и `MIN_RESPONSE_TOKENS = 64`) строго `<= 35` токенов; итоговый `token_count <= effective_max_tokens`.
- Длина `src/cli/setup/rule_template.md` строго `<= 25` строк; корневой `AGENTS.md` обязан оставаться `<= 22 000` байт; все JSON/MD файлы заканчиваются `\n` (`0x0A`).
- Перед коммитом каждой задачи проходят `rtk cargo fmt --all -- --check`, `rtk cargo clippy --workspace --all-targets -- -D warnings` и `rtk cargo test --workspace`; коммит создаётся через `rtk oco -y` (или конвенциональный commit).

## Review Focus

1. Исходный код содержит литерал `]]>` внутри кода и спецсимволы `&`, `"`, `<` в пути/сводке — `PackStyle::Xml` экранирует `]]>` как `]]]]><![CDATA[>` и атрибуты XML, оставаясь валидным для `quick_xml::Reader`; тест в Task 1 (`test_pack_xml_escapes_cdata_terminator_and_attributes`).
2. В `L0_index.json` или фильтре `files` передан симлинк внутри репозитория, ведущий наружу, либо паттерн с обратным слешем/диском (`..\\secret.env`) на POSIX — `pack_subsystem`/`pack_files` отклоняют симлинки и опасные паттерны с ошибкой, но пропускают легитимный код вроде `CredentialsProvider.kt` и `secret_rotation.rs`; тест в Task 2 (`test_pack_rejects_symlinks_backslashes_and_allows_secret_named_code`).
3. Многобайтовый UTF-8 (кириллический BSL-код и 4-байтовые эмодзи `🦀`) попадает ровно на границу сплита при `budget = 64` и `budget = 300`, либо агент передаёт `cur_...` после смены `files`/`style` — `paginate_content_with` не режет UTF-8 кодовые точки, удерживает `token_count <= budget` на малых бюджетах с компактным футером и отклоняет курсор с несовпадающим `query_hash`; тест в Task 3 (`test_paginate_multibyte_utf8_boundary_and_query_hash_mismatch`).
4. Kotlin-файл содержит raw-строки `"""... { ..."""`, строковую интерполяцию `"${foo()}"` и expression-body функции `fun calc(x: Int) = x * 2` без `{}` (либо подсистема состоит только из `.md`/`.toml` без кода) — `KotlinAdapter` не сбивает глубину скобок, а `PackMode::Toc` выдаёт непустой индекс с `[Lstart-Lend]` и заголовками `.md`/`.toml`; тест в Task 4 (`test_kotlin_raw_strings_expression_bodies_and_non_code_toc_fallback`).
5. Вызов CLI `context-init pack <id> --style markdown --max-lines 25` разрезает вывод внутри открытого блока ```` ```rust ```` — пагинатор резервирует строки под закрывающий ```` ``` ```` и строку курсора `--cursor "cur_..."`, оставляя итоговое число строк строго `<= 25`; тест в Task 5 (`cli_pack_max_lines_closes_code_fence_within_budget`).

---

## Карта файлов

| Файл | Ответственность | Задачи |
| :--- | :--- | :--- |
| `crates/ci-core/src/analysis/pack.rs` | `PackStyle::Compact`, `PackMode`, `CompressOptions`, `PackRequest`, `pack_subsystem_with`, `pack_files_with`, безопасность путей, `is_secret_path`, экранирование XML, рендер TOC | Tasks 1, 2, 4 |
| `crates/ci-core/src/analysis/lang/{mod,rust,kotlin,brace,python,bsl}.rs` | `SymbolSpan`, `LanguageAdapter::{compress_with, extract_symbol_spans}`, фикс `"ts"` в `brace.rs`, `method_bodies` и raw-строки/expression-body | Task 4 |
| `crates/ci-mcp/src/tools.rs` | Расширение `McpPackParams` (`files`, `symbols`, `mode`) | Task 2 |
| `crates/ci-mcp/src/pagination.rs` | `CursorData::query_hash`, `FooterContext`, `format_footer_ex`, оконный UTF-8-безопасный сплит | Task 3 |
| `crates/ci-mcp/src/handlers.rs` | Проброс `PackRequest` в `ContextHandler::pack`, проверка `PACK_ALL_MAX_PAGES` после фильтра, исключение двойного `live_source_tokens` | Tasks 2, 3, 8 |
| `crates/ci-mcp/src/server.rs` | Передача новых полей `McpPackParams`, маппинг ошибок параметров в `ErrorData::invalid_params`, `ServerConfig::instructions` | Tasks 1, 2, 5 |
| `src/cli/pack.rs`, `src/cli/mod.rs` | Дефолт `"compact"`, флаги `--files`/`--filter`/`--symbols`/`--mode`/`--toc`/`--max-lines`/`--max-tokens`/`--cursor`, лестница авто-деградации | Tasks 1, 2, 5 |
| `src/cli/setup/{rule_template.md,rules.rs}`, `crates/ci-core/src/analysis/generator.rs`, `.agents/skills/context-init/SKILL.md` | Обновление правил агентов, `render_root_agents_md`, `SKILL.md` (синхронно с появлением флагов) | Task 5 |
| `crates/ci-core/src/git.rs` | Аддитивное расширение `RepoGitSnapshot` (`tree_entries`, `root_tree_sha`, `unborn_head`), `git ls-tree -r -t -z HEAD`, паритет `blob_sha` и `paths_tree_sha` | Task 6 |
| `crates/ci-core/src/analysis/generator.rs` | Проброс `&RepoGitSnapshot` и `CardCache` в `update_with_options`, `audit`, `card_content`, группировка `diff_name_only` | Task 6 |
| `crates/ci-core/src/analysis/{scope,scan,detector,subsystems,cache}.rs` | Единый `ScopeFilter::build_with_diagnostic`, 1x `File::open` в `scan`, удаление `thread_local! SCOPE`, `scope.files()` в `detector`, `AnalyzedFileCache` + внешний дисковый кэш в `cache_dir` | Tasks 7, 8 |
| `crates/ci-mcp/src/pipeline.rs` | Точечный `refresh_stale_card` с fallback на `.mdo` и `parent.is_some()`, проброс `CardCache`/`RepoGitSnapshot` в `run_check` | Tasks 6, 8 |
| `crates/ci-mcp/src/lib.rs`, `tests/cli.rs`, `tests/regression_benchmark.rs` | Адаптация и расширение unit/integration/benchmark тестов | Tasks 1–8 |

---

### Task 1: `PackStyle::Compact`, `PackMode`, `CompressOptions`, экранирование XML и совместимость `compress_source`

**Files:**
- Modify: `crates/ci-core/src/analysis/pack.rs:10-210`
- Modify: `crates/ci-mcp/src/server.rs:470-478`
- Modify: `src/cli/pack.rs:33-53`, `src/cli/mod.rs:284-293`
- Test: `crates/ci-core/src/analysis/pack.rs`, `tests/cli.rs:236-251`, `tests/regression_benchmark.rs:280-291`

**Interfaces:**
- Produces:
  - `pub enum PackStyle { #[default] Compact, Markdown, Xml }` с `PackStyle::try_parse(&str) -> Result<Self>` и `PackStyle::parse(&str) -> Self` в `ci_core::analysis::pack`
  - `pub enum PackMode { Full, #[default] Compressed, Toc }` в `ci_core::analysis::pack`
  - `pub struct CompressOptions { pub strip_imports: bool, pub public_only: bool, pub exclude_tests: bool, pub strip_comments: bool, pub inline_elide: bool }` (`legacy()` и `compact()`) в `ci_core::analysis::pack`
  - `pub struct PackRequest<'a>` и `pub fn compress_source_with(content: &str, ext: &str, opts: &CompressOptions) -> String`
- Consumes: `ci_core::analysis::lang::adapter_for_ext`, `quick_xml`.

- [ ] **Step 1: Написать падающие тесты на `PackStyle::Compact`, `try_parse`, legacy-совместимость `compress_source` и экранирование XML**

В `crates/ci-core/src/analysis/pack.rs` (`mod packer_tests`) добавить 3 теста:
1. `test_pack_style_try_parse_rejects_unknown`: `PackStyle::try_parse("compact") == Ok(PackStyle::Compact)`, `PackStyle::try_parse("YAML").is_err()`.
2. `test_pack_compact_format_has_no_xml_or_markdown_banners`: вызвать `pack_files(..., PackStyle::Compact)` и проверить наличие `=== lib.rs (1L) ===`, отсутствие `<?xml`, `<![CDATA[`, `# 📦 AI Context Pack`.
3. `test_pack_xml_escapes_cdata_terminator_and_attributes`: передать файл с именем `a&"b<c.rs` и кодом `let s = "]]>";`, вызвать `pack_files(..., PackStyle::Xml)`, распарсить результат через `quick_xml::Reader` до `Eof` без ошибок и проверить сохранение текста `"]]>"`.
В `tests/cli.rs` обновить `cli_pack_compress_outputs_xml` (передав `--style xml`) и добавить `cli_pack_compress_outputs_compact_by_default`, проверяющий `stdout(&out).starts_with("=== ")`. В `src/cli/mod.rs:289` обновить ожидание дефолта `assert_eq!(a.style, "compact");`.

- [ ] **Step 2: Запустить тесты и убедиться, что они падают**

Run: `rtk cargo test -p ci-core -- test_pack_style_try_parse test_pack_compact_format test_pack_xml_escapes`  
Expected: FAIL (`PackStyle::Compact` и `try_parse` не определены).

- [ ] **Step 3: Реализовать `PackStyle::Compact`, `PackMode`, `CompressOptions`, `PackRequest`, экранирование XML и переключить дефолт на `"compact"`**

1. В `crates/ci-core/src/analysis/pack.rs`: объявить `PackStyle` (`Compact` как `#[default]`, `Markdown`, `Xml`), `PackMode`, `CompressOptions` (`legacy()`, `compact()`), `PackRequest<'a>` и `compress_source_with(content, ext, opts)` (временно делегирующую в `compress_source` до Task 4). Оставить `compress_source(content, ext)` без изменений.
2. В `pack_files`: для `PackStyle::Compact` выводить заголовок `=== {file_rel} ({line_count}L) ===\n{processed}\n\n`; для `PackStyle::Xml` экранировать атрибуты `id`, `name`, `path` и заменять `]]>` в теле на `]]]]><![CDATA[>`.
3. В `crates/ci-mcp/src/server.rs:474` и `src/cli/pack.rs:37` изменить значение по умолчанию на `"compact"`, а валидацию стиля перевести на `PackStyle::try_parse` (в MCP возвращая `rmcp::ErrorData::invalid_params`, в CLI — `anyhow` ошибку). Обновить `McpPackParams::style` docstring в `tools.rs:102`.

- [ ] **Step 4: Проверить прохождение тестов workspace**

Run: `rtk cargo test --workspace`  
Expected: PASS (включая `src/cli/mod.rs:284`, `tests/cli.rs` и `tests/regression_benchmark.rs`).

- [ ] **Step 5: Коммит**

```bash
rtk git add crates/ci-core/src/analysis/pack.rs crates/ci-mcp/src/server.rs crates/ci-mcp/src/tools.rs src/cli/pack.rs src/cli/mod.rs tests/cli.rs
rtk oco -y
```

---

### Task 2: Точечная фильтрация `files` и `symbols` с защитой путей и симлинков

**Files:**
- Modify: `crates/ci-core/src/analysis/pack.rs:42-199`
- Modify: `crates/ci-mcp/src/tools.rs:90-108`
- Modify: `crates/ci-mcp/src/handlers.rs:540-616`
- Modify: `crates/ci-mcp/src/server.rs:463-480`
- Modify: `src/cli/pack.rs:18-55`
- Test: `crates/ci-core/src/analysis/pack.rs`, `crates/ci-mcp/src/lib.rs`, `tests/cli.rs`

**Interfaces:**
- Produces:
  - `pub fn pack_subsystem_with(root: &Path, knowledge_dir: &Path, index: &L0Index, req: &PackRequest<'_>) -> Result<PackedSubsystems>` в `ci_core::analysis::pack`
  - `pub fn pack_files_with(root: &Path, subs: &[&SubsystemInfo], files: &[String], req: &PackRequest<'_>) -> String` в `ci_core::analysis::pack`
  - поля `files: Option<Vec<String>>`, `symbols: Option<Vec<String>>`, `mode: Option<String>` в `McpPackParams`
  - `ContextHandler::pack_with(project: &ResolvedProject, req: &PackHandlerParams<'_>, tel: &TelemetryScope) -> Result<String>` (или расширение `ContextHandler::pack`)
- Consumes: `PackStyle`, `PackMode`, `CompressOptions`, `PackRequest` из Task 1; `globset::GlobSet`; `ci_core::storage::safe_join`.

- [ ] **Step 1: Написать падающие тесты на фильтрацию `files`, ошибку при 0 совпадений, `PACK_ALL_MAX_PAGES`, симлинки и `is_secret_path`**

1. В `crates/ci-core/src/analysis/pack.rs` (`mod pack_subsystem_tests`):
   - `test_pack_filters_by_glob_and_short_filename`: подсистема из 3 файлов (`src/a.rs`, `src/nested/target.rs`, `src/b.rs`); запрос `files = &["target.rs"]` возвращает только `src/nested/target.rs`; `files = &["missing.rs"]` возвращает `Err` со списком доступных файлов.
   - `test_pack_rejects_symlinks_backslashes_and_allows_secret_named_code`: создать внутри корня легитимные `src/CredentialsProvider.kt`, `src/secret_rotation.rs`, а также `credentials.json`, `secrets.yaml`, `.npmrc` и Unix-симлинк `src/link.rs -> /etc/hosts` (под `#[cfg(unix)]`). Убедиться, что:
     - `CredentialsProvider.kt` и `secret_rotation.rs` успешно упакованы;
     - `credentials.json`, `secrets.yaml`, `.npmrc` отсечены `is_secret_path`;
     - симлинк `src/link.rs` не прочитан и не включён;
     - паттерны `..\\foo.env`, `/etc/passwd`, `C:\\foo.rs`, `[invalid` возвращают ошибку валидации.
2. В `crates/ci-mcp/src/lib.rs`: тест `pack_all_with_files_filter_applies_before_page_limit`, проверяющий, что `context_pack(subsystem_id="all", files=["target.rs"], compress=false)` на крупном проекте успешно возвращает файл, а не отказ `PACK_ALL_MAX_PAGES`.

- [ ] **Step 2: Запустить тесты и убедиться, что они падают**

Run: `rtk cargo test -p ci-core -- test_pack_filters_by_glob test_pack_rejects_symlinks`  
Expected: FAIL (`pack_subsystem_with` и фильтрация отсутствуют).

- [ ] **Step 3: Реализовать `pack_subsystem_with`, валидацию `files`/`is_secret_path` и проброс в MCP/CLI**

1. В `pack.rs`: реализовать валидатор паттернов `validate_file_patterns(&[String])` (запрет пустых, `\0`, ведущих `/` или `\`, `[A-Za-z]:`, компонента `..`, компиляция `globset::GlobSet` с явной ошибкой).
2. Реализовать `pack_subsystem_with`: отбор кандидатов из подсистемы (или всех подсистем при `subsystem_id == "all"` и непустом `files`), матчинг по полному пути / суффиксу `/{pat}` / относительно `sub_dir`, ошибка с перечнем файлов при 0 совпадений.
3. В `pack_subsystem_with` и `pack_files_with` проверять `!is_secret_path(p)`, `safe_join(root, p)`, `!fs::symlink_metadata(&full)?.file_type().is_symlink()`, `fs::canonicalize(&full)?.starts_with(&canonical_root)`. Расширить `is_secret_path`: `*.p12`, `*.pfx`, `*.jks`, `*.keystore`, `id_rsa*`, `id_ed25519*`, `.npmrc`, `.netrc`, `secrets.*`, `credentials.json`, `google-services.json`. Оставить `pack_subsystem` / `pack_files` обёртками над `*_with`.
4. В `tools.rs:90`: добавить `files`, `symbols`, `mode` (все `#[serde(default)]`) в `McpPackParams`, обновить старые литералы `McpPackParams` в `crates/ci-mcp/src/lib.rs:1596, 2098, 2129, 2279, 2468` добавлением `..Default::default()`.
5. В `handlers.rs` / `server.rs`: ошибки валидации `files` / `style` / `mode` пробрасывать как `rmcp::ErrorData::invalid_params`; проверку `PACK_ALL_MAX_PAGES` (`handlers.rs:570`) выполнять только когда `files.is_empty()` или уже после фильтрации; добавить `--files` / `--filter`, `--symbols`, `--mode` в `src/cli/pack.rs`.

- [ ] **Step 4: Проверить прохождение тестов `ci-core`, `ci-mcp` и CLI**

Run: `rtk cargo test --workspace`  
Expected: PASS.

- [ ] **Step 5: Коммит**

```bash
rtk git add crates/ci-core/src/analysis/pack.rs crates/ci-mcp/src/tools.rs crates/ci-mcp/src/handlers.rs crates/ci-mcp/src/server.rs crates/ci-mcp/src/lib.rs src/cli/pack.rs
rtk oco -y
```

---

### Task 3: Адаптивный футер пагинации, `CursorData::query_hash` и UTF-8-безопасный оконный сплит

**Files:**
- Modify: `crates/ci-mcp/src/pagination.rs:10-350`
- Modify: `crates/ci-mcp/src/handlers.rs:406, 456, 520, 601`
- Test: `crates/ci-mcp/src/pagination.rs`, `crates/ci-mcp/src/lib.rs`

**Interfaces:**
- Produces:
  - `CursorData { pub byte_offset: usize, pub page: usize, pub query_hash: Option<u64> }` в `ci_mcp::pagination`
  - `pub struct PaginationMeta<'a> { pub query_hash: Option<u64>, pub remaining_sections: &'a [String] }`
  - `pub fn paginate_content_with(full_text: &str, cursor: Option<&str>, max_tokens: usize, stale: bool, meta: &PaginationMeta<'_>) -> Result<PaginatedOutput, String>`
  - `pub fn format_footer_ex(...) -> String` (с сохранением `format_footer` для `budget < 256` и старых тестов)
- Consumes: `ci_core::tokens::{bpe, count_tokens}`, `sha2::Sha256`.

- [ ] **Step 1: Написать падающие тесты на UTF-8 сплит, `query_hash` mismatch, малый бюджет `60..64` и богатый футер `>= 256`**

В `crates/ci-mcp/src/pagination.rs` (`mod tests`) добавить:
1. `test_paginate_multibyte_utf8_boundary_and_query_hash_mismatch`:
   - сгенерировать строку из кириллических BSL-строк (`// Комментарий: ОбработатьДанные()\n`) и 4-байтовых эмодзи (`// 🦀 Обработка\n`), проверить пагинацию на `budget = 64` и `budget = 300`: полная склейка `body` всех страниц посимвольно равна исходной строке, `out.token_count <= budget` на каждой странице;
   - проверить, что передача `next_cursor` от `meta_a` в вызов с другим текстом/`query_hash` (`meta_b`) возвращает ошибку несовпадения курсора.
2. `test_format_footer_respects_budget_tiers`: при `budget = 64` футер укладывается в `<= 35` токенов и содержит `[truncated: true | next_cursor: "cur_`; при `budget = 4000` и списке `remaining_sections` футер содержит `page: 1/`, `remaining:` и подсказку `next:`.

- [ ] **Step 2: Запустить тесты и убедиться, что они падают**

Run: `rtk cargo test -p ci-mcp -- test_paginate_multibyte_utf8 test_format_footer_respects_budget`  
Expected: FAIL (`query_hash`, `paginate_content_with`, `PaginationMeta` не существуют).

- [ ] **Step 3: Реализовать `query_hash`, двухуровневый `format_footer_ex` и оконный поиск сплита**

1. В `pagination.rs`: добавить `#[serde(default, skip_serializing_if = "Option::is_none")] pub query_hash: Option<u64>` в `CursorData` (64-битный хеш из `Sha256` от параметров + полного текста). При несовпадении `Some(h)` в `paginate_content_with` возвращать явную ошибку (маппится в `invalid_params` в MCP).
2. Реализовать `format_footer_ex`: если `effective_max_tokens < 256`, использовать точный компактный шаблон `format_footer` (`<= 35` ток.); если `>= 256` — включать `page: {page}/{est_total}`, `remaining: ... (+K)` (собирая оставшиеся заголовки `=== file` / `## File:` за `next_offset`) и короткую подсказку вызова `cursor` / `files`.
3. В `find_split_point`: заменить бинарный поиск по всему тексту на ограниченное байтовое окно `window = remaining[..find_valid_char_boundary(remaining, (target_body_tokens * 6).max(512).min(remaining.len()))]` (удваивая окно только при нехватке токенов в окне), выполнять бинарный поиск по `\n` и валидным UTF-8 границам внутри окна, никогда не вызывая `bpe.decode(&tokens[..k])` напрямую на срезе токенов. Оставить `paginate_content(...)` совместимой обёрткой.
4. В `handlers.rs::pack`: использовать `paginate_content_with` с `query_hash` (от `subsystem_id`, `mode`, `style`, `files`, `symbols`, длины/хеша контента) и списком оставшихся файлов.

- [ ] **Step 4: Проверить прохождение тестов `ci-mcp`**

Run: `rtk cargo test -p ci-mcp`  
Expected: PASS (включая `budget = 60` и `MIN_RESPONSE_TOKENS = 64` в `crates/ci-mcp/src/lib.rs:2059`).

- [ ] **Step 5: Коммит**

```bash
rtk git add crates/ci-mcp/src/pagination.rs crates/ci-mcp/src/handlers.rs
rtk oco -y
```

---

### Task 4: Расширение `LanguageAdapter` (`compress_with`, `SymbolSpan`), фикс адаптеров и режим L1.5 `PackMode::Toc`

**Files:**
- Modify: `crates/ci-core/src/analysis/lang/mod.rs:19-40`
- Modify: `crates/ci-core/src/analysis/lang/{rust,kotlin,brace,python,bsl}.rs`
- Modify: `crates/ci-core/src/analysis/pack.rs` (подключить `compress_source_with`, выборку `symbols`, рендер `PackMode::Toc`)
- Test: `crates/ci-core/src/analysis/lang/mod.rs`, `crates/ci-core/src/analysis/lang/kotlin.rs`, `crates/ci-core/src/analysis/lang/brace.rs`, `crates/ci-core/src/analysis/pack.rs`

**Interfaces:**
- Produces:
  - `pub struct SymbolSpan { pub kind: &'static str, pub signature: String, pub is_public: bool, pub line_start: usize, pub line_end: usize }` в `ci_core::analysis::lang`
  - `LanguageAdapter::compress_with(&self, source: &str, ext: &str, opts: &CompressOptions) -> String`
  - `LanguageAdapter::extract_symbol_spans(&self, source: &str, ext: &str) -> Vec<SymbolSpan>`
  - `pack_files_with` поддерживает `PackMode::Toc` и непустой `PackRequest::symbols`
- Consumes: `CompressOptions`, `PackMode` из Task 1; `has_cfg_test`, `is_syn_pub`, `extract_multiline_symbols` из `detector.rs`.

- [ ] **Step 1: Написать падающие тесты на `compress_with`, `SymbolSpan`, фикс `"ts"` в `BraceLangAdapter`, Kotlin raw/expression-body и TOC для не-кодовых файлов**

1. В `crates/ci-core/src/analysis/lang/brace.rs`: тест `brace_extract_symbols_and_compress_respect_go_and_ts_extensions` — проверка извлечения Go `func Serve(...)` и TypeScript `export interface ...` без хардкода `"ts"`, схлопывания импортов при `CompressOptions::compact()` и непустого `method_bodies`.
2. В `crates/ci-core/src/analysis/lang/kotlin.rs`: тест `test_kotlin_raw_strings_expression_bodies_and_non_code_toc_fallback` — файл с `"""raw { string"""`, `"${nested()}"`, `fun calc(x: Int) = x * 2`, `private fun helper() {}` и `class Repo { fun load() { ... } }`: проверка `extract_symbol_spans` (`[Lstart-Lend]` не сбиваются), `compress_with(..., compact)` (схлопывает `import`, сжимает `{ ⋮ }`) и `PackMode::Toc` для `.md` (заголовки `#`/`##`) и `.toml` (`[package]`, `[dependencies]`).
3. В `crates/ci-core/src/analysis/pack.rs`: тесты `test_rust_compress_with_compact_strips_tests_imports_and_inlines_elide` (сверяет экономию токенов и отсутствие `#[cfg(test)]` при `exclude_tests = true`, сохраняя старый `test_compress_rust_ast_with_attributes_and_references` зелёным!) и `test_pack_symbols_extracts_exact_method_bodies`.

- [ ] **Step 2: Запустить тесты и убедиться, что они падают**

Run: `rtk cargo test -p ci-core -- test_kotlin_raw_strings brace_extract_symbols test_rust_compress_with_compact test_pack_symbols`  
Expected: FAIL (`SymbolSpan`, `compress_with`, `extract_symbol_spans` ещё не реализованы).

- [ ] **Step 3: Реализовать `SymbolSpan`, `compress_with`, фиксы адаптеров, `symbols` и `PackMode::Toc`**

1. В `lang/mod.rs`: определить `SymbolSpan`, методы `compress_with` и `extract_symbol_spans` с безопасными дефолтами в `LanguageAdapter`.
2. В `lang/rust.rs` и `pack.rs`: реализовать `compress_rust_ast_with(content, opts)` (подсчёт `use` -> `// [N use omitted]`, фильтрация `#[cfg(test)]` через `has_cfg_test`, `public_only` через `is_syn_pub`, `strip_comments`, однострочное `{ ⋮ }`) и 1-проходный `extract_rust_symbol_spans` (определение точных `[line_start-line_end]` через таблицу смещений строк и лексическое сканирование границ `{}` деклараций без добавления `syn/span-locations` в `Cargo.toml`).
3. В `lang/brace.rs`: исправить `brace.rs:22` — передавать реальный `ext` в `extract_multiline_symbols` (для `extract_symbols` использовать нейтральный fallback, а в `extract_symbol_spans` / `compress_with` — переданный `ext`), реализовать `method_bodies`, `extract_symbol_spans` и схлопывание преамбул `import`/`package`/`using`.
4. В `lang/kotlin.rs`, `python.rs`, `bsl.rs`: реализовать `compress_with` (с учётом `strip_imports`, `strip_comments`, `public_only`, `inline_elide`), `extract_symbol_spans` и `method_bodies` для Kotlin (с поддержкой `"""..."""`, `"${...}"`, expression-body `fun ... = ...`).
5. В `pack.rs`: подключить `compress_source_with` в `pack_files_with`; при `PackRequest::symbols` отбирать только совпадающие `method_bodies`/`SymbolSpan`; при `PackMode::Toc` рендерить `[TOC] {id} ...` со строками символов `[Lstart-Lend]` и fallback для `.md`/`.toml`/`.yaml`/`.json`/`.sql`/`.sh` (`file (NL)` + заголовки/ключи).

- [ ] **Step 4: Проверить прохождение тестов `ci-core`**

Run: `rtk cargo test -p ci-core`  
Expected: PASS (включая замер экономии токенов `compact` vs `xml` ≥ 40%).

- [ ] **Step 5: Коммит**

```bash
rtk git add crates/ci-core/src/analysis/lang/ crates/ci-core/src/analysis/pack.rs
rtk oco -y
```

---

### Task 5: Бюджет CLI `pack` (`--max-lines`/`--max-tokens`/`--cursor`), `ServerConfig.instructions` и синхронизация шаблонов

**Files:**
- Modify: `src/cli/pack.rs:18-110`
- Modify: `crates/ci-mcp/src/server.rs:694-704`
- Modify: `src/cli/setup/rule_template.md`, `src/cli/setup/rules.rs:28-97`
- Modify: `crates/ci-core/src/analysis/generator.rs:1057-1067`
- Modify: `.agents/skills/context-init/SKILL.md`
- Test: `tests/cli.rs`, `src/cli/setup/rules.rs`, `crates/ci-mcp/src/lib.rs`

**Interfaces:**
- Produces:
  - `PackArgs` поля `max_lines: Option<usize>`, `max_tokens: Option<usize>`, `cursor: Option<String>`, `toc: bool`, `signatures_only: bool`, `public_only: bool`, `keep_imports: bool`, `include_tests: bool` в `src/cli/pack.rs`
  - функция деградации бюджета `apply_cli_budget(...) -> Result<String>` в `src/cli/pack.rs`
  - `instructions: Some(...)` в `ContextInitServer::get_info` (`crates/ci-mcp/src/server.rs:695`)
- Consumes: `PackMode`, `PackStyle`, `CompressOptions`, `pack_subsystem_with` (Tasks 1, 2, 4); `ci_mcp::pagination::paginate_content_with` (Task 3).

- [ ] **Step 1: Написать падающие тесты на `--max-lines` (с закрытием code fence), `--cursor`, `ServerConfig.instructions` и размер `rule_template.md`**

1. В `tests/cli.rs`:
   - `cli_pack_max_lines_closes_code_fence_within_budget`: запустить `pack <id> . --style markdown --no-compress --max-lines 25`, проверить `stdout(&out).lines().count() <= 25`, сбалансированность маркеров ```` ``` ```` (чётное количество) и наличие подсказки `--cursor "cur_..."`, затем добрать следующую страницу с `--cursor`.
   - `cli_pack_max_lines_degrades_full_to_compact_and_toc`: проверить переключение `full -> compact -> toc` при жёстком `--max-lines 40` без флага `--no-compress`.
2. В `crates/ci-mcp/src/lib.rs`: тест `server_get_info_provides_instructions` проверяет `server.get_info().instructions.is_some()` и наличие `context_pack`, `path`, `cursor`.
3. В `src/cli/setup/rules.rs`: расширить `rule_template_is_at_most_25_lines` проверкой упоминания `compact`, `toc`, `files`, `cursor` при сохранении `lines <= 25`.

- [ ] **Step 2: Запустить тесты и убедиться, что они падают**

Run: `rtk cargo test -- cli_pack_max_lines server_get_info_provides_instructions`  
Expected: FAIL (флаги `--max-lines`/`--cursor` и `instructions` ещё не добавлены).

- [ ] **Step 3: Реализовать бюджет CLI, `ServerConfig.instructions` и обновить шаблоны инструкций**

1. В `src/cli/pack.rs`: добавить флаги `PackArgs`; если вывод идёт в `stdout` и заданы `--max-lines` или `--max-tokens`, применить лестницу `full -> compact -> toc -> paginate` (для `--toc`/`--signatures-only` сразу `PackMode::Toc`, а при явных `--files` + `--no-compress` не сбрасывать в `Toc`, а переходить к пагинации). При построчной нарезке под `--max-lines` вычислять открытый code fence (```` ``` ````) на границе среза и резервировать 1 строку под закрывающий fence + 1 строку под `--cursor "cur_..."` **до** обрезки тела. При `-o <file>` писать полный результат без деградации, если лимиты явно не заданы.
2. В `crates/ci-mcp/src/server.rs:695`: установить `info.instructions = Some(...)` с маршрутом `context_get_subsystems` → `context_pack(mode="toc")` → `context_pack(files=[...], compress=false)`, обязательным `path` и правилом добора по `cursor`.
3. Синхронно обновить:
   - `src/cli/setup/rule_template.md` (`<= 25` строк, готовые JSON-подсказки вызовов без чтения lazy-схем, сохранение всех маркеров тестов `rules.rs:74-96`);
   - `crates/ci-core/src/analysis/generator.rs:1058-1065` (`render_root_agents_md`, сохраняя общий размер `AGENTS.md <= 22 000` байт);
   - `.agents/skills/context-init/SKILL.md` (исправить `.agents/knowledge/**` в строке 20 на `.context-init/knowledge/**`, убрать `-o /tmp/context.xml`, описать `compact`/`toc`/`files`/`cursor`/`--max-lines`).

- [ ] **Step 4: Проверить прохождение тестов workspace**

Run: `rtk cargo test --workspace`  
Expected: PASS.

- [ ] **Step 5: Коммит**

```bash
rtk git add src/cli/pack.rs crates/ci-mcp/src/server.rs src/cli/setup/rule_template.md src/cli/setup/rules.rs crates/ci-core/src/analysis/generator.rs .agents/skills/context-init/SKILL.md tests/cli.rs crates/ci-mcp/src/lib.rs
rtk oco -y
```

---

### Task 6: Батчинг Git (`RepoGitSnapshot` + `git ls-tree`) и `CardCache` в `generator` / `pipeline`

**Files:**
- Modify: `crates/ci-core/src/git.rs:35-130, 414-515`
- Modify: `crates/ci-core/src/analysis/generator.rs:75-172, 243-252, 416-515, 561-728`
- Modify: `crates/ci-mcp/src/pipeline.rs:140-191`
- Test: `crates/ci-core/src/git.rs`, `crates/ci-core/src/analysis/generator.rs`

**Interfaces:**
- Produces:
  - расширение `RepoGitSnapshot` полями `#[serde(default, skip)] tree_entries: BTreeMap<String, GitTreeEntry>`, `root_tree_sha: Option<String>`, `unborn_head: bool` (при сохранении `head_commit` и `status_entries`) + методы `snapshot.blob_sha_for(&self, repo_root, rel_path)` и `snapshot.paths_tree_sha(&self, repo_root, paths)`
  - тип `CardCache = BTreeMap<String, String>` и `*_with_snapshot` перегрузки внутри `generator.rs`
- Consumes: `SystemGitBackend`, `check_card_staleness_with_snapshot` в `crates/ci-core/src/git.rs`.

- [ ] **Step 1: Написать падающие тесты на паритет `RepoGitSnapshot` (`ls-tree`) и батчинг в `git.rs` / `generator.rs`**

В `crates/ci-core/src/git.rs` (`mod tests`) добавить тесты:
1. `snapshot_tree_shas_match_blob_sha_on_dirs_root_dirty_files_and_unborn_head`:
   - на пустом git-репозитории без коммитов (unborn `HEAD`) `repo_snapshot` не падает (`unborn_head == true`);
   - после коммита `src/a.rs`, `src/sub/b.rs`: сравнить `snapshot.blob_sha_for` с `git.blob_sha` для `"."`, `"src"`, `"src/a.rs"`, `paths_tree_sha(&["src/a.rs", "src/sub/b.rs"])`;
   - после локальной неиндексированной правки `src/a.rs` и добавления untracked `src/new.rs`: проверить, что single-file `snapshot.blob_sha_for` возвращает актуальный `hash-object`, совпадающий с `git.blob_sha`!
2. `snapshot_parses_ls_tree_with_submodules_and_tabs_safely`: парсер `parse_ls_tree_z` корректно обрабатывает записи вида `160000 commit <sha>\tvendor/sub\0` и пути с не-ASCII символами/пробелами.

- [ ] **Step 2: Запустить тесты и убедиться, что они падают**

Run: `rtk cargo test -p ci-core -- snapshot_tree_shas_match snapshot_parses_ls_tree`  
Expected: FAIL (`tree_entries`, `blob_sha_for`, `parse_ls_tree_z` не существуют).

- [ ] **Step 3: Реализовать батчинг `RepoGitSnapshot` и проброс `CardCache` / `&RepoGitSnapshot` в `generator` и `pipeline`**

1. В `git.rs`: реализовать `parse_ls_tree_z` (сплит записи строго по `\t`, сохранение `object_type`, игнорирование падений на unborn `HEAD` через `unborn_head = true`), 1 вызов `git rev-parse --verify HEAD^{tree}` для `root_tree_sha` и 1 вызов `git ls-tree -r -t -z HEAD` внутри `SystemGitBackend::repo_snapshot`. В `blob_sha_for`: для `"."` брать `root_tree_sha`; для single-file с записью в `status_entries` или при `unborn_head` вызывать `hash-object`; для остальных путей брать из `tree_entries`. Реализовать `snapshot.paths_tree_sha` в памяти.
2. В `generator.rs`: построить `RepoGitSnapshot` (если внутри Git) и `CardCache` **один раз** в начале `update_with_options` и `audit`; передавать их в `card_is_pristine`, `card_content`, `render_card_for`, `is_git_stale`; группировать `diff_name_only` по уникальным `card.source.commit_sha` в одном запуске.
3. В `pipeline.rs::run_check`: переиспользовать `CardCache` между `audit` и циклом `check_subsystem_staleness_with_snap` и единый `snapshot`.

- [ ] **Step 4: Проверить прохождение тестов `ci-core` и `ci-mcp`**

Run: `rtk cargo test --workspace`  
Expected: PASS.

- [ ] **Step 5: Коммит**

```bash
rtk git add crates/ci-core/src/git.rs crates/ci-core/src/analysis/generator.rs crates/ci-mcp/src/pipeline.rs
rtk oco -y
```

---

### Task 7: Единый обход ФС (`scope`/`scan`/`detector`), удаление `thread_local! SCOPE` и внешний дисковый кэш метрик

**Files:**
- Create: `crates/ci-core/src/analysis/cache.rs`
- Modify: `crates/ci-core/src/analysis/mod.rs`
- Modify: `crates/ci-core/src/analysis/scope.rs:132-265`
- Modify: `crates/ci-core/src/analysis/scan.rs:21-51, 140-260`
- Modify: `crates/ci-core/src/analysis/detector.rs:88-200, 820-1830`
- Test: `crates/ci-core/src/analysis/cache.rs`, `crates/ci-core/src/analysis/scan.rs`, `crates/ci-core/src/analysis/detector/tests.rs`

**Interfaces:**
- Produces:
  - `pub struct AnalyzedFileCache` и `pub struct DiskMetricsCache` (сохраняется строго в `StoragePaths::cache_dir/<project_hash>/metrics_v1.json`) в `ci_core::analysis::cache`
  - `ScopeFilter::build_with_diagnostic(root: &Path, scope: &ScopeConfig) -> Result<(Self, Option<Self>)>` (или единый проход для `scan`) в `ci_core::analysis::scope`
  - `pub detected_token_map: BTreeMap<String, usize>` (или в составе `ProjectAnalysis` / кэша) для передачи посчитанных токенов в `subsystems::build_deterministic`
- Consumes: `StoragePaths::cache_dir` (`ci_core::config`), `sha2::Sha256`, `ci_core::VERSION`.

- [ ] **Step 1: Написать падающие тесты на `DiskMetricsCache`, изоляцию от repo-дерева, `scope.files()` в `detector` и целостность `l0_omits_volatile_fields`**

1. В `crates/ci-core/src/analysis/cache.rs`: тест `disk_metrics_cache_lives_outside_repo_and_invalidates_on_content_change` — проверяет загрузку/сохранение в temp `cache_dir`, промах при изменении содержимого файла с сохранением длины в ту же секунду (за счёт проверки `sha256` или `ctime`/хэша для свежих файлов) и инвалидацию при смене `ci_core_version`.
2. В `crates/ci-core/src/analysis/detector/tests.rs`: тест `detector_respects_scope_without_thread_local_across_threads`, проверяющий, что вызов `analyze_project_with` из двух потоков с разными `ScopeConfig` одновременно не протекает через глобальное состояние.
3. Проверить сохранение зелёного `l0_omits_volatile_fields` (`analysis/mod.rs:109`).

- [ ] **Step 2: Запустить тесты и убедиться, что они падают**

Run: `rtk cargo test -p ci-core -- disk_metrics_cache detector_respects_scope`  
Expected: FAIL (`cache.rs` не создан).

- [ ] **Step 3: Реализовать единый обход ФС, удаление `thread_local! SCOPE` и `AnalyzedFileCache`/`DiskMetricsCache`**

1. В `scope.rs` и `scan.rs`: объединить двойной `WalkBuilder` в `scan_project_with` в единый проход; в `scan.rs` убрать отдельный `File::open` в `is_likely_binary` — читать 8 КБ заголовок из уже открытого файла перед дочиткой до `TOKENIZE_MAX_BYTES`.
2. В `detector.rs`: полностью удалить `thread_local! static SCOPE` и `ScopeGuard`, пробросив `&ScopeFilter` явно во все функции `discover_subsystems` / `contains_relevant_files_depth` / `collect_files_recursive_excluding`, переводя сбор файлов на срез `scope_filter.files()` и индексированные карты (включая замену $O(K^2)$ `symbols.contains` на `BTreeSet` и удаление `ancestors().find_map`).
3. Создать `crates/ci-core/src/analysis/cache.rs`: единый `AnalyzedFileCache` на время запуска (объединяет оба `FileMemo`, `count_lines`, `estimate_tokens_fast` и символы при 1 чтении файла) + опциональный `DiskMetricsCache` в `StoragePaths::cache_dir` (ключ: `rel_path` -> `len`, `modified_nanos`, `sha256_hex`, `ci_core_version`, `lines`, `bpe_tokens`, `symbols`). Передать кэшированные токены в `ProjectAnalysis` (с `#[serde(skip)]`) для использования в Task 8.

- [ ] **Step 4: Проверить прохождение тестов `ci-core` и `l0_omits_volatile_fields`**

Run: `rtk cargo test -p ci-core`  
Expected: PASS.

- [ ] **Step 5: Коммит**

```bash
rtk git add crates/ci-core/src/analysis/cache.rs crates/ci-core/src/analysis/mod.rs crates/ci-core/src/analysis/scope.rs crates/ci-core/src/analysis/scan.rs crates/ci-core/src/analysis/detector.rs crates/ci-core/src/analysis/detector/tests.rs
rtk oco -y
```

---

### Task 8: Устранение лишнего BPE в `subsystems`/`handlers`, точечный `refresh_stale_card` и финальная сдача

**Files:**
- Modify: `crates/ci-core/src/analysis/subsystems.rs:95-142`
- Modify: `crates/ci-mcp/src/handlers.rs:357-425, 560-605`
- Modify: `crates/ci-mcp/src/pipeline.rs:206-218`
- Test: `crates/ci-mcp/src/lib.rs`, `tests/regression_benchmark.rs`

**Interfaces:**
- Consumes: `AnalyzedFileCache` / предвычисленные токены `ProjectAnalysis` (Task 7), `RepoGitSnapshot` (Task 6).
- Produces: точечный `refresh_stale_card` с безопасным fallback на полный `run_update` для `.mdo`, `sub.parent.is_some()`, сплита `>= DEFAULT_SPLIT_THRESHOLD_TOKENS` и появления новых манифестов.

- [ ] **Step 1: Написать падающие тесты на исключение лишнего BPE и безопасный `refresh_stale_card`**

1. В `crates/ci-mcp/src/lib.rs`:
   - `get_subsystems_telemetry_reuses_computed_tokens_without_double_scan`: проверить корректность `EfficiencyEvent.source_tokens` == сумме `tokens` из `L0SubsystemSummary` при активной телеметрии.
   - `refresh_stale_card_handles_single_crate_and_falls_back_for_split_or_mdo`: изменить один файл в простой подсистеме, вызвать `refresh_stale_card` и проверить обновление frontmatter SHA и актуальность `check_subsystem_staleness`; отдельно проверить безопасный fallback при `sub.parent.is_some()`.

- [ ] **Step 2: Запустить тесты и убедиться, что они падают**

Run: `rtk cargo test -p ci-mcp -- get_subsystems_telemetry_reuses refresh_stale_card_handles`  
Expected: FAIL.

- [ ] **Step 3: Оптимизировать `subsystems::build_deterministic`, `handlers.rs` и `refresh_stale_card`**

1. В `subsystems::build_deterministic` (`subsystems.rs:96`): использовать предвычисленные токены из `ProjectAnalysis` / `AnalyzedFileCache` без повторного чтения всех файлов с диска.
2. В `handlers::get_subsystems` (`handlers.rs:376, 414`): при наличии валидного `DiskMetricsCache` и отсутствии `stale` брать токены из кэша; при активной телеметрии суммировать уже посчитанные `summaries.iter().map(|s| s.tokens).sum()` вместо повторного вызова `live_source_tokens`. В `handlers::pack` переиспользовать `DiskMetricsCache` для `live_source_tokens`.
3. В `pipeline.rs::refresh_stale_card` (`L209`): выполнять локальный пересчёт символов/токенов/SHA только для файлов `sub` и обновление одной L1-карточки, делая fallback на полный пересчёт при `sub.parent.is_some()`, наличии `.mdo` файлов, пересечении `DEFAULT_SPLIT_THRESHOLD_TOKENS` или изменении набора манифестов. В `handlers::get_subsystem` (`L468-474`) не перепроверять staleness второй раз сразу после успешного `refresh_stale_card`.

- [ ] **Step 4: Полная верификация, обновление собственной базы знаний и бенчмарк**

Run:
```bash
rtk cargo fmt --all -- --check
rtk cargo clippy --workspace --all-targets -- -D warnings
rtk cargo test --workspace
rtk cargo run -- update . --prune --compact
rtk cargo run -- check . --compact --exit-code
```
Expected: 0 ошибок форматирования, 0 предупреждений `clippy`, 100% зелёные тесты, `check` выводит `OK:` (`0 drift`), размер `AGENTS.md <= 22 000` байт.

- [ ] **Step 5: Финальный коммит**

```bash
rtk git add crates/ci-core/src/analysis/subsystems.rs crates/ci-mcp/src/handlers.rs crates/ci-mcp/src/pipeline.rs crates/ci-mcp/src/lib.rs .context-init/knowledge AGENTS.md
rtk oco -y
```
