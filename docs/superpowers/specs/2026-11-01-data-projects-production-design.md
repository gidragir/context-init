# ТЕХНИЧЕСКАЯ СПЕЦИФИКАЦИЯ (RFC): Доведение `context-init` до промышленной эксплуатации в `/data/projects` и устранение узких мест бенчмарка

> **Статус:** Unified Production Readiness RFC (Handoff + Empirical Audit of 29 `/data/projects` directories)  
> **Объект аудита:** `context-init` v0.3.0 (`ci-core`, `ci-llm`, `ci-mcp`, CLI)  
> **Источники фактических данных:** `reports/benchmark_latest.json` + эмпирический прогон `scan`, `init --dry-run`, `check`, `workspace` и живых MCP-вызовов на всех 29 директориях `/data/projects` + продакшен-телеметрия (`~/.local/share/context-init/telemetry/2026-11.jsonl`, 69 событий).

---

## 0. Сводный архитектурный диагноз (Handoff + `/data/projects`)

Объединенный аудит выявил **5 взаимосвязанных архитектурных блокеров**, объясняющих как метрики регрессионного бенчмарка (`reports/benchmark_latest.json`), так и отказы на реальных репозиториях `/data/projects`:

| № | Архитектурный блокер | Фактическая причина в коде | Масштаб поражения |
| :--- | :--- | :--- | :--- |
| **1** | **Сбой разрешения корня проекта, схемы `L0Index` и диагностики устаревшего бинарника в `ci-mcp`** | 1) Глобальный MCP стартует в `$HOME` (`/home/gidragir`) без `workspace_roots = ["/data/projects"]`.<br>2) [`resolve_project_root`](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/server.rs#L148-L160) вызывает [`find_project_root_upward`](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/server.rs#L335-L345) на несуществующем относительном пути `cwd.join("walle")`, возвращая чужой проект.<br>3) [`L0Index::from_json_str`](file:///home/gidragir/projects/context-init/crates/ci-core/src/storage.rs#L479-L485) падает на `found > CURRENT_L0_SCHEMA_VERSION` с ложным требованием `init --force`, а [`instrumented`](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/server.rs#L248-L266) не добавляет [`stale_binary_footer`](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/server.rs#L83-L87) к ветке `Err(e)`. | **100% вызовов MCP** без абсолютного `path` или после пересборки CLI |
| **2** | **Шумовое загрязнение токенов и подсистем** | 1) Lock-файлы (`pnpm-lock.yaml`, `Cargo.lock`, `uv.lock`, `.tsbuildinfo`) не исключены в [`ScopeFilter::build`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/scope.rs#L43-L109) (занимают **64–93% токенов**).<br>2) Бинарники $> 512\text{ KB}$ оцениваются по `size / 4` до проверки UTF-8 в [`scan_project`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/scan.rs#L73-L82) (**14.58M** токенов в `wasmbridge-projects`).<br>3) Скрытые `.agent/skills` (`walle` -> 50 подсистем) и чужие `.git` (`docs/OpenViking` -> 415 подсистем) не отсекаются. | **21 из 25 репозиториев (84%)** в `/data/projects` |
| **3** | **Слепота детектора на глубине 1 и глубине > 4** | 1) [`contains_relevant_files`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/detector.rs#L930-L944) проверяет только глубину 1 (`fs::read_dir`), теряя `infra/` в `ai-dotfiles`, `libs/` в `nestjs-zkbio-cvsecurity`, `home/` в `dotfiles`, `dot_config/` в `win-dotfiles`.<br>2) [`collect_files_recursive`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/detector.rs#L859) обрезает обход на `max_depth = 4`, теряя глубокие пакеты Java (`cfe-updater`), Kotlin (`mimir`), формы 1С (`walle`) и Stow (`ai-dotfiles`).<br>3) Не распознаются `.astro`, Gradle (`build.gradle.kts`) и Maven (`pom.xml`). | **12 из 25 репозиториев (48%)** в `/data/projects` |
| **4** | **Отсутствие AST-компрессии TS/JS/Kotlin/Java и Python + промах BM25 (`recall@5 = 0.90`)** | 1) В [`lang::all()`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/lang/mod.rs#L34-L36) нет адаптера для TS/JS/Kotlin/Java (`0%` сжатия), а [`PythonAdapter::compress_source`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/lang/python.rs) дает только `9%`.<br>2) Жадный [`SplitContext::split`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/detector.rs#L1140-L1159) поглощает `tests/regression_benchmark.rs` в родительский `tests`, а [`Bm25Retriever::search`](file:///home/gidragir/projects/context-init/crates/ci-core/src/search.rs#L211-L215) безусловно умножает `score` подсистем с тегом `"tests"` на `TESTS_SCORE_FACTOR = 0.35`, даже когда запрос прямо совпадает с `id`/`path` тестовой подсистемы. | **18 из 25 репозиториев (72%)** + промах в `bench_search_quality_and_latency` |
| **5** | **Холодный старт `120–136 ms` в бенчмарке и `531–816 ms` в реальном CLI `check` + последовательный `workspace`** | 1) Первая итерация `analyze_project` / `audit` инициализирует BPE-словарь [`tiktoken_rs::cl100k_base()`](file:///home/gidragir/projects/context-init/crates/ci-core/src/tokens.rs#L10-L15) (`~110 ms`) внутри [`tokens_if_over`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/detector.rs#L970-L992).<br>2) `head_commit` и `remote_url` в [`SystemGitBackend`](file:///home/gidragir/projects/context-init/crates/ci-core/src/git.rs#L163-L215) делают fork/exec `git`, а [`pipeline::run_check`](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/pipeline.rs#L139-L144) спавнит $3 \times N$ процессов `git` в цикле (`~450 ms`).<br>3) [`discover_workspace`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/workspace.rs#L108) опрашивает репозитории однопоточно и не проходит по симлинкам. | Латентность CLI `check` **p50 = 531–755 ms**; холодный старт бенчмарка **120–136 ms** |

---

## РАЗДЕЛ 1. Полная диагностическая матрица всех 29 директорий `/data/projects`

| № | Директория в `/data/projects` | Стек (Факт -> Детектор) | Токены `scan` (Шум) | Подсистем (`init`) | Эмпирически зафиксированные дефекты текущего `context-init` |
| :- | :--- | :--- | :--- | :--- | :--- |
| 1 | **`ai-dotfiles`** | Ansible / Stow / Shell -> `Generic` | `127,484` | 15 | **Потерян весь `infra/` (112 файлов, 51.3% репо: `infra/stow`, `infra/playbooks`)** из-за глубины 1 в [`contains_relevant_files`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/detector.rs#L930-L944) и `max_depth = 4` в [`collect_files_recursive`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/detector.rs#L859). Одиночные файлы `services_hermes_agents` (`AGENTS.md`) и 5 планов в `docs/superpowers/plans/*` стали подсистемами. Падает `check` (база в `.agents/knowledge`). |
| 2 | **`anki-english`** | React + Vite + TS -> `TypeScript` | `198,270` (**92.1% шум**) | 3 | `public/words.json` (`182,584` ток.) выделен в самостоятельную кодовую подсистему `public_words`; `src/` сжат в 1 подсистему с 0% AST-компрессии TS. |
| 3 | **`cfe-updater`** | Java (Maven) -> **`Generic` (ошибка)** | `24,090` | 3 | Не распознан `pom.xml` -> `Generic`. Из-за `max_depth = 4` в [`collect_files_recursive`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/detector.rs#L859) пакеты глубже `src/main/java/ru/...` теряются; `.github` выделен в подсистему. |
| 4 | **`context-init`** | Rust Workspace -> `Rust` | `322,315` (`Cargo.lock` 14%) | 34 | Жадный [`SplitContext::split`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/detector.rs#L1140-L1159) выделил `tests_cli`, но поглотил `tests/regression_benchmark.rs` в родительский `tests` + штраф `TESTS_SCORE_FACTOR = 0.35` в [`search.rs`](file:///home/gidragir/projects/context-init/crates/ci-core/src/search.rs#L88) дает промах на `"benchmark regression"`. |
| 5 | **`dotfiles`** | Shell / Stow -> `Generic` | `97,114` | 2 (`bin`, `docs`) | **Потерян весь `home/` (179 файлов, 77.8% репо)** из-за глубины 1 в [`contains_relevant_files`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/detector.rs#L930-L944). Падает `check` (база в `.agents/knowledge`). |
| 6 | **`gidragir`** | TS + Submodules -> `TypeScript` | `2,609,465` (**94.9% шум**) | 8 | Бинарные файлы в `public/` оценены по `size / 4` в **2,377,969 токенов** (91.2%); потерян `submodules/` (35 файлов). |
| 7 | **`git-sync`** | Tauri + React + Rust -> `TypeScript` | `124,028` (**58.4% шум**) | 9 | `Cargo.lock` + `pnpm-lock.yaml` раздувают токены; 4 JSON-схемы `apps/desktop/src-tauri/gen/schemas/*.json` стали отдельными подсистемами. |
| 8 | **`gitlab-cli`** | Rust -> `Rust` | `30,089` (**83.1% шум**) | 2 | `Cargo.lock` = `25,017` токенов (**83.1%** всего проекта). |
| 9 | **`glazewm`** | Rust Workspace -> `Rust` | `2,342,356` (**85.2% шум**) | 6 | 16 бинарных ресурсов в `resources/` оценены в **1,994,952 токенов**; `packages_wm-platform_readme` (`README.md`) стал отдельной подсистемой; `description` равен `"<div align=\"center\">"`. |
| 10 | **`hermes-helper`** | Rust + Profiles -> `Rust` | `24,144` (**45.1% шум**) | 2 (`crates`, `scripts`) | Потеряны `profiles/` (12 файлов) и `plugins/idea-capture/` (6 файлов) из-за глубины 1 в [`contains_relevant_files`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/detector.rs#L930-L944); `Cargo.lock` = `10,889` ток. (45.1%). |
| 11 | **`jupyter`** | Python (uv) -> `Python` | `30,965` (**93.4% шум**) | 1 (`ipynb_checkpoints`) | `uv.lock` = `28,912` ток. (**93.4%**); единственная найденная подсистема — мусорный кэш `.ipynb_checkpoints`. |
| 12 | **`mimir`** | Kotlin Multiplatform (Gradle) -> **`TypeScript` (ошибка)** | `151,148` | 8 | Ошибочно классифицирован как `TypeScript` вместо `Kotlin`; глубокие пакеты `src/main/kotlin/com/...` обрезаются на `max_depth = 4`; `.github` стал подсистемой. |
| 13 | **`nestjs-zkbio-cvsecurity`** | NestJS Library -> `TypeScript` | `30,052` | **1** (`scripts`) | **Потерян весь основной код `libs/zkbio-client` (51 файл, 99.0% репо)**. |
| 14 | **`obsidian-orbit`** | Obsidian Plugin (TS) -> `TypeScript` | `46,374` | 5 | Падает `check` (база в legacy `.agents/knowledge`); 0% AST-компрессии для TypeScript в `context_pack`. |
| 15 | **`obsidian-sample-plugin`** | Obsidian Plugin (TS) -> `TypeScript` | `22,553` (**92.5% шум**) | 0 | `pnpm-lock.yaml` = `20,860` ток. (**92.5%**); `main.ts` в корне проекта не образует подсистему (`0` подсистем). |
| 16 | **`omni-dotfiles`** | Dotfiles -> `Generic` | `54,485` | 4 | **Потерян весь `linked/` (65 файлов, 53.1% репо)** из-за глубины 1 в [`contains_relevant_files`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/detector.rs#L930-L944). |
| 17 | **`openrgb-mcp`** | Rust MCP -> `Rust` | `15,436` (**79.6% шум**) | 1 (`src`) | `Cargo.lock` = `12,285` ток. (**79.6%**); в `workspace` ветка отображается как `?` (unborn HEAD). |
| 18 | **`parent-watch`** | TS / React / Tauri -> `TypeScript` | `289,472` (**75.1% шум**) | 32 | `pnpm-lock.yaml` = `187,370` ток. (**64.7%**); созданы мусорные подсистемы `.changeset`, `.deploy` и 12 подсистем из `apps/frontend/.agent/skills/*`. |
| 19 | **`resume_1c`** | Astro + TS -> `TypeScript` | `72,855` (**87.8% шум**) | 1 (`src_content`) | `pnpm-lock.yaml` = `63,944` ток. (**87.8%**). **Потеряны все UI-компоненты `src/components/*.astro`, `src/pages/*.astro`, `src/layouts/*.astro`**. |
| 20 | **`snippet-templater`** | Obsidian Plugin (TS) -> `TypeScript` | `29,828` (**90.7% шум**) | 1 (`src`) | `pnpm-lock.yaml` = `27,056` ток. (**90.7%**); 0% AST-компрессии TS. |
| 21 | **`Tab-clip-manager`** | Tauri + TS -> `TypeScript` | `78,763` (**89.2% шум**) | 2 | `pnpm-lock.yaml` (`47,764`) + `Cargo.lock` (`22,520`) = **89.2% всех токенов**. |
| 22 | **`tiktok`** | Python -> `Python` | `11,240` | 2 | Не является git-репозиторием; `PythonAdapter::compress_source` дает только `9%` сжатия. |
| 23 | **`vacansies-galaxy`** | Tauri + React + Rust -> `TypeScript` | `125,082` (**85.9% шум**) | 4 | `Cargo.lock` (`80,592`) + `pnpm-lock.yaml` (`26,880`) = **85.9% всех токенов**. |
| 24 | **`walle`** | 1C (BSL/MDO) + Python -> `OneC` | `590,340` (**66.4% шум**) | **68** (**50 мусорных**) | Скрытый `.agent/skills/*` (`392,169` ток.) порождает 45+ подсистем; `.claude` и `tests_readme` стали подсистемами; глубокие модули форм 1С теряются из-за `max_depth = 4`. |
| 25 | **`wasmbridge-projects`** | Rust / C# / 1C -> `Rust` | `23,803,023` (**98.7% шум**) | **428** (**422 мусорных**) | 1) Вложенный git-клон `docs/OpenViking` (`8.88M` ток., 2082 файла) разбит на ~415 подсистем. 2) 11 бинарников в `installer/` оценены по `size / 4` в **14,581,641 токенов**. 3) Не виден в `workspace` (6 дочерних git-репозиториев). |
| 26 | **`win-dotfiles`** | Chezmoi / PowerShell -> `Generic` | `14,040` | **0** | **Потерян весь `dot_config/` (61 файл, 98.2% репо)** (`0` подсистем). |
| 27 | **`zkbio-cvs`** | TS / Tauri / Rust -> `TypeScript` | `181,964` (**86.8% шум**) | 9 | `pnpm-lock.yaml` (`85,878`) + `.tsbuildinfo` (`49,321`) + `Cargo.lock` (`22,685`) = **86.8% всех токенов**; `.changeset` и `.github` выделены в подсистемы. |
| 28 | **`.cargo-cache` / `.pnpm-store` / `.sccache` / `.uv-cache`** | Кэши сборки в `/data/projects` | — | — | Требуют отсечения в `IGNORED_DIRS` и `discover_workspace`. |
| 29 | **`.Trash-1000`** | Системная корзина раздела `/data` | — | — | Требует отсечения в `IGNORED_DIRS` и `discover_workspace`. |

---

## РАЗДЕЛ 2. Целевая архитектура и 5 технических треков реализации

### Трек 1: Гигиена `ScopeFilter` и `scan_project` (`crates/ci-core/src/analysis/{scope,scan,detector}.rs`)

1. **Исключение lock-файлов и сгенерированных бандлов (`IGNORED_LOCK_AND_ARTIFACT_FILES`, `IGNORED_FILE_SUFFIXES`):**
   - `Cargo.lock`, `pnpm-lock.yaml`, `package-lock.json`, `yarn.lock`, `bun.lock`, `bun.lockb`, `uv.lock`, `poetry.lock`, `Pipfile.lock`, `composer.lock`, `Gemfile.lock`, `flake.lock`, `skills-lock.json`, `lazy-lock.json`.
   - Суффиксы: `.tsbuildinfo`, `.min.js`, `.min.css`, `.bundle.js`, `.js.map`, `.css.map`, `.d.ts.map`.
2. **Отсечение скрытых dot-директорий (`name.starts_with('.')`) и вложенных Git-репозиториев на `depth > 0`:**
   - Любая директория с `name.starts_with('.')` (если не включена явно в `scope.include`) и любая вложенная директория с собственным `.git` (если не является workspace-членом корня) отсекаются в [`ScopeFilter::build`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/scope.rs#L52-L65).
3. **Детекция бинарных файлов до оценки `size / 4` в [`scan_project`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/scan.rs#L73-L82):**
   - Функция `is_likely_binary(path: &Path) -> bool` проверяет бинарные расширения и первые `8192` байт на наличие `0x00` (`NUL`) или фатальной ошибки UTF-8 (`err.error_len().is_some()`), возвращая `(0, 0)` токенов.

---

### Трек 2: Полнота детектора, быстрый подсчет токенов без `tiktoken_rs` и симметричный `SplitContext` (`crates/ci-core/src/analysis/detector.rs`)

1. **Глубокое обнаружение подсистем (`max_depth = 12` и контейнеры `infra`/`libs`/`home`/`linked`/`dot_config`):**
   - Заменить [`contains_relevant_files`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/detector.rs#L930-L944) на рекурсивную проверку (до глубины 8) и поднять `max_depth` в [`collect_files_recursive`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/detector.rs#L859) с `4` до `12`.
   - Расширить [`common_parent`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/detector.rs#L698-L700) контейнерами `"libs"`, `"plugins"`, `"profiles"`, `"infra"`, `"home"`, `"linked"`, `"dot_config"`, `"submodules"`.
   - Добавить поддержку `.astro`, `.vue`, `.svelte`, `.kt`, `.kts`, `.ps1`, `.psm1`, `.lua`, `.nu`, `.css`, `.scss`, `.prisma`, `.graphql`, `.proto`, а также детекцию Gradle (`build.gradle.kts`, `settings.gradle.kts`, `build.gradle`) и Maven (`pom.xml`) в [`detect_type_and_name`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/detector.rs#L172-L216) и очистку HTML-тегов в [`extract_description`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/detector.rs#L434-L451).
2. **Разделение `SOURCE_CODE_EXTENSIONS` vs `CONFIG_AND_DOC_EXTENSIONS` и симметричный [`SplitContext::split`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/detector.rs#L1122-L1216):**
   - Не-кодовые файлы (`.md`, `.json`, `.yaml`, `.toml`, `.xml`) помечаются `pinned = true` и никогда не выделяются в одиночные файловые подсистемы (`tests_readme`, `public_words`).
   - Если в директории без `pinned` кодового entrypoint (например `tests/`) хотя бы один кодовый файл превысил порог и выделился в `<id>_<stem>` (`tests_cli`), все соседние кодовые файлы этой директории с размером $\ge 300$ токенов также выделяются в собственные подсистемы (`tests_regression_benchmark`).
3. **Устранение холодного старта `tiktoken_rs` (`120 ms`) в [`tokens_if_over`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/detector.rs#L970-L992):**
   - Сначала проверять суммарный размер файлов в байтах (`total_bytes < threshold * 3 -> None`), затем использовать быстрый детерминированный эстиматор `estimate_tokens_fast(text: &str) -> usize` без вызова `tiktoken_rs::cl100k_base()`.

---

### Трек 3: Мультиязычная AST-компрессия и калибровка двуязычного BM25 поиска (`crates/ci-core/src/analysis/lang`, `crates/ci-core/src/search.rs`)

1. **Zero-Dependency `BraceLangAdapter` и `PythonAdapter::compress_source`:**
   - Создать `crates/ci-core/src/analysis/lang/brace.rs` (`BraceLangAdapter` для `ts`, `tsx`, `js`, `jsx`, `mjs`, `cjs`, `mts`, `cts`, `astro`, `vue`, `svelte`, `kt`, `kts`, `java`, `cs`, `go`): извлечение публичных сигнатур и замена тел функций/методов `{ ... }` на [`ELIDED_BODY`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/lang/mod.rs#L15) (`⋮----`) с сохранением `interface`/`type` и игнорированием скобок в строках и комментариях.
   - Доработать [`PythonAdapter::compress_source`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/lang/python.rs) (вырезание индентированных тел `def`/`async def` с сохранением сигнатур и docstrings, экономия $\ge 65\%$).
2. **Двухэтапное ранжирование и калибровка BM25 в [`crates/ci-core/src/search.rs`](file:///home/gidragir/projects/context-init/crates/ci-core/src/search.rs#L85-L229):**
   - Сохранить двуязычный стеммер `rust_stemmers` (`Algorithm::Russian` + `Algorithm::English`) в [`stems()`](file:///home/gidragir/projects/context-init/crates/ci-core/src/search.rs#L32-L66) и добавить в `Entry` предвычисленное множество `id_stems: HashSet<String>` (стемы `sub.id` и `sub.path`).
   - **Условный `TESTS_SCORE_FACTOR`:** применять понижающий коэффициент `TESTS_SCORE_FACTOR = 0.35` **только если запрос НЕ пересекается с `id_stems` тестовой подсистемы и не содержит тестовых/бенчмарк-термов** (`"test"`, `"bench"`, `"benchmark"`, `"regression"`).
   - **Буст точного совпадения идентификатора/пути и сглаживание длины карточки:** умножать `score` на `1.0 + 0.6 * (matched_id_stems as f32)`, устраняя доминирование коротких карточек `reports` над целевыми кодовыми модулями (`recall@5 = 1.00`).

---

### Трек 4: Zero-Fork Fast-Path Git (`head_commit`, `remote_url`, `.git` gitfile) и батчинг `RepoGitSnapshot` (`crates/ci-core/src/git.rs`, `crates/ci-mcp/src/pipeline.rs`)

1. **In-process парсинг `.git/HEAD`, `.git/packed-refs` и `.git/config` (`head_commit` и `remote_url`):**
   - Реализовать `resolve_git_dir(repo_root: &Path) -> Option<PathBuf>`: если `.git` — директория, вернуть её; если `.git` — файл (worktree или submodule вида `gitdir: ../.git/modules/...`), распарсить путь `gitdir:`.
   - В [`SystemGitBackend::head_commit`](file:///home/gidragir/projects/context-init/crates/ci-core/src/git.rs#L163-L181): читать `HEAD` -> `ref: refs/heads/...` -> файл рефа или `packed-refs` напрямую из процесса (0 fork/exec), с фолбэком на `git rev-parse HEAD`.
   - В [`SystemGitBackend::remote_url`](file:///home/gidragir/projects/context-init/crates/ci-core/src/git.rs#L183-L215): парсить секцию `[remote "origin"]` (или первый `[remote "..."]`) из `<git_dir>/config` напрямую в памяти (0 fork/exec), с фолбэком на `git config --get`.
2. **Батчинг проверки устаревания карточек (`RepoGitSnapshot`):**
   - Ввести `RepoGitSnapshot` в [`GitBackend`](file:///home/gidragir/projects/context-init/crates/ci-core/src/git.rs#L78-L115), собирающий `head_commit` (0 спавнов), `status_porcelain_z` (1 спавн `git status --porcelain=v1 -z`) и карту всех tree/blob SHA `HEAD` (1 спавн `git ls-tree -r -t HEAD`).
   - Перевести [`pipeline::run_check`](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/pipeline.rs#L134-L148) на `check_subsystem_staleness_with_snapshot`, сокращая число спавнов `git` с $3 \times N$ (`~450 ms`) до 2 вызовов (`< 15 ms`).
   - В [`storage_mismatch_hint`](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/pipeline.rs#L197-L215) детектировать legacy-базы `.agents/knowledge`.

---

### Трек 5: Мульти-проектный MCP-рантайм, параллельный `discover_workspace` и forward-compatibility схемы (`crates/ci-core/src/{config,storage,analysis/workspace}.rs`, `crates/ci-mcp/src/server.rs`)

1. **`workspace_roots` и многоуровневый [`resolve_project_root`](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/server.rs#L130-L201) / [`child_repo`](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/server.rs#L311-L345):**
   - Добавить `workspace_roots: Vec<PathBuf>` в [`Config`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L24-L66) с дефолтом `/data/projects` и `$HOME/projects`.
   - Вызывать [`find_project_root_upward`](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/server.rs#L335-L345) только при `candidate.exists()`; в `child_repo` искать в `<roots>/<name>`, `<roots_parent>/<name>` и `<workspace_roots>/<name>`.
2. **Forward-compatible схема в [`L0Index::from_json_str`](file:///home/gidragir/projects/context-init/crates/ci-core/src/storage.rs#L471-L502) и [`stale_binary_footer`](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/server.rs#L83-L87) на ошибках:**
   - При `found > CURRENT_L0_SCHEMA_VERSION` десериализовать `L0Index` в режиме прямой совместимости без `bail!`.
   - В [`ContextInitServer::instrumented`](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/server.rs#L248-L266) добавлять `stale_binary_footer()` также к текстам ошибок `Err(e)`, чтобы агент и пользователь видели уведомление о перезапуске MCP даже при ошибке вызова.
3. **Параллельный и симлинк-совместимый [`discover_workspace`](file:///home/gidragir/projects/context-init/crates/ci-core/src/analysis/workspace.rs#L89-L148):**
   - Заменить `e.file_type().is_ok_and(|t| t.is_dir())` на `e.path().is_dir()` (проход по симлинкам `~/projects -> /data/projects`), проверять дочерние репозитории на глубине +1 (`wasmbridge-projects/*`) и опрашивать статус репозиториев параллельно через `std::thread::scope`.

---

## РАЗДЕЛ 3. Матрица верификации и целевые SLO (Бенчмарк + `/data/projects`)

| Компонент / Метрика | Текущая метрика | Целевой SLO | Тестовый гейт |
| :--- | :--- | :--- | :--- |
| **Cold-Start Drift Audit Latency** | `120–136 ms` (1-я итерация) | **$< 15.0\text{ ms}$** (1-я итерация), **$< 6.0\text{ ms}$** (`avg`) | `bench_drift_audit_latency` в [`tests/regression_benchmark.rs`](file:///home/gidragir/projects/context-init/tests/regression_benchmark.rs) |
| **Real CLI `context-init check` Latency** | `531–816 ms` (`telemetry/2026-11.jsonl`) | **$< 25.0\text{ ms}$** | `rtk context-init check /data/projects/context-init --compact` |
| **BM25 Search Precision (`recall@5`)** | `0.90` (промах на `"benchmark regression"`) | **`1.00` (`recall@5 = 1.0`)** при `avg < 1.0 ms` | `bench_search_quality_and_latency` в [`tests/regression_benchmark.rs`](file:///home/gidragir/projects/context-init/tests/regression_benchmark.rs#L497-L560) |
| **AST-компрессия (TS/JS/Kotlin/Java и Python)** | TS/JS = `0%`, Python = `9.0%` | **$\ge 65.0\%$** экономии токенов (Rust $\ge 70\%$, BSL $\ge 70\%$) | `bench_ast_compression_ratio` + `typescript_and_python_adapters_compress_bodies_over_65_percent` |
| **Гигиена токенов и подсистем в `/data/projects`** | Lock-файлы = 64–93% токенов; `walle` = 68 подсистем; `wasmbridge-projects` = 428 подсистем (`23.8M` ток.) | Lock-файлы и бинарники = **0 токенов**; `walle` $\le 18$ подсистем; `wasmbridge-projects` $\le 15$ подсистем | `rtk context-init init <repo> --dry-run --role init=none` |
| **Полнота детектора на `/data/projects`** | Потеряны `infra/` (`ai-dotfiles`), `libs/` (`nestjs-zkbio-cvsecurity`), `.astro` (`resume_1c`), `dot_config/` (`win-dotfiles`) | **100% присутствие** всех целевых подсистем и верный `project_type` (`Kotlin`/`Java`) | Юнит-тесты в `detector.rs` + dry-run на проектах `/data/projects` |
| **MCP: Разрешение проектов и forward-compat схема** | Сбой из `$HOME`, возврат чужого проекта для `walle`, отказ на `schema v4` | **100% разрешение** любого проекта из `/data/projects` + чтение `schema v4+` | Юнит-тесты в `server.rs` и `storage.rs` + живой MCP-вызов |
