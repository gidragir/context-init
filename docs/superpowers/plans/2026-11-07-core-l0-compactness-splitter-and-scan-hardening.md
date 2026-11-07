# context-init: Компактность L0, универсальная гранулярность детектора и защита `scan`/`update` — план реализации

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Сократить ответ `context_get_subsystems` до чистой L0-проекции, устранить атомизацию плоских папок и потерю файлов контейнеров в детекторе, защитить `sanitize_id` от коллизий, вычистить частные Stow-хардкоды с починкой префиксного обхода `[scope].include`, защитить `context_scan` от Context Bombs и внедрить `update --prune` и `kb_status` в `context_workspace`.

**Architecture:** Обобщение базовых алгоритмов в `crates/ci-core` (`scope.rs`, `file_kind.rs`, `detector.rs`, `generator.rs`, `scan.rs`, `workspace.rs`) без частных топологических профилей, с проекцией DTO и пробросом параметров в `crates/ci-mcp` и `src/cli`. Схема хранения `L0_index.json` остаётся `v4`.

**Tech Stack:** Rust 2021, `ci-core`, `ci-mcp`, `serde`, `serde_json`, `globset`, `ignore`, `sha2`, `clap`, `rtk`.

**Spec:** `docs/superpowers/specs/2026-11-07-core-l0-compactness-splitter-and-scan-hardening-design.md`

## Global Constraints

- Все терминальные команды выполняются строго с префиксом `rtk` (`rtk cargo ...`, `rtk git ...`); GNU `find` и `grep` запрещены (использовать только `rtk fd` и `rtk rg` без флага `-E`).
- Для навигации субагентов по существующим модулям `> 1 000` токенов использовать `rtk context-init pack <subsystem_id> --compress` (или MCP `context_pack`), не вычитывая каталоги целиком.
- Схема `L0_index.json` остаётся `CURRENT_L0_SCHEMA_VERSION = 4`; старые `L0_index.json` с `"stow_packages": []` и старые `.context-init.toml` со `stow = false` в `[integrations]` обязаны десериализоваться без ошибок.
- Все `subsystem_id` обязаны удовлетворять `SUBSYSTEM_ID_PATTERN` (`^[a-z0-9][a-z0-9_-]{0,63}$`, длина `1..=64`).
- Каталог `.git` блокируется на любой глубине безусловно (даже при явном `[scope].include` и `include_hidden = true`); `follow_links(false)` неизменен.
- Перед коммитом каждой задачи должны проходить `rtk cargo test --workspace` и `rtk cargo clippy --workspace --all-targets -- -D warnings`.

## Review Focus

1. Сводка подсистемы содержит переводы строк `\n` и кавычки `"` — построчная сборка массива `L0SubsystemSummary` экранирует их через `serde_json::to_string`, оставляя весь ответ валидным JSON для `serde_json::from_str`; тест в Task 1 (`get_subsystems_fits_65_entries_and_escapes_multiline_summary`).
2. Путь длиннее 64 символов имеет серию `_` или `-` ровно на границе 55-го символа — `sanitize_id` обрезает хвостовые `_-` перед добавлением `_{8hex}`, исключая двойной `__` и превышение 64 символов; тест в Task 2 (`test_sanitize_id_long_paths_hash_and_boundary_separators`).
3. Пользователь задал глобальную маску `[scope] include = ["**/*.toml"]` — `include_allows_dir_traversal` не открывает скрытые папки `.git`, `.agents` или `node_modules` от ведущих джокеров `**`, спускаясь в dot-директории только по явному префиксу пути (`.config/**`); тест в Task 3 (`scope_include_prefix_descends_into_explicit_dot_dir_but_not_wildcard`).
4. Контейнер монорепо (`packages/`) содержит прямой файл `packages/tsconfig.base.json` и подпроект `packages/ui/src/index.ts` — остаточный модуль `packages` включает только `packages/tsconfig.base.json` и не дублирует файлы `packages/ui`; тест в Task 4 (`test_container_keeps_direct_files_without_duplicating_child_modules`).
5. Репозиторий состоит на 100% из файлов-выбросов (`clean_total_tokens == 0`) — `scan_project_with` не порождает `NaN`/`Inf` при делении долей и сериализуется в валидный JSON с пустым `suggested_scope.extensions`; тест в Task 5 (`scan_all_outliers_does_not_produce_nan_shares`).

---

## Карта файлов

| Файл | Ответственность | Задачи |
| :--- | :--- | :--- |
| `crates/ci-mcp/src/tools.rs` | DTO `L0SubsystemSummary`, расширение `McpScanParams`, `McpUpdateParams`, `McpWorkspaceParams` | Tasks 1, 5, 6, 7 |
| `crates/ci-mcp/src/handlers.rs` | Компактный рендер `get_subsystems`, проброс опций `scan`, `update(prune)`, `workspace(paths, kb_status)`, подсказка `check` | Tasks 1, 5, 6, 7 |
| `crates/ci-mcp/src/server.rs` | Передача новых полей `McpScanParams`, `McpUpdateParams`, `McpWorkspaceParams` в `ContextHandler` | Tasks 5, 6, 7 |
| `crates/ci-core/src/analysis/generator.rs` | Хеш-суффикс `sanitize_id`, дедупликация `normalize_index` и `AGENTS.md`, `update_with_options(..., prune)` | Tasks 2, 6 |
| `crates/ci-core/src/analysis/scope.rs` | Удаление `is_stow_or_dotfiles`, префиксный спуск `include_allows_dir_traversal`, синхронизация с `file_kind`, режим `include_hidden` | Tasks 3, 5 |
| `crates/ci-core/src/storage.rs` | Удаление мёртвого поля `L0Index::stow_packages` | Task 3 |
| `crates/ci-core/src/config.rs` | Удаление публичного `stow` с тихой совместимостью `_legacy_stow` в `PartialIntegrationsConfig` | Task 3 |
| `crates/ci-core/src/analysis/detector.rs` | Порог `1 000` токенов, удаление каскадного выселения, `is_flat_file_collection`, фикс `tests/`, остаточные файлы контейнеров, развязка коллизий ID | Tasks 2, 4, 5 |
| `crates/ci-core/src/analysis/file_kind.rs` (новый) | Единая нормализация манифестов (`Dockerfile.*`, `Modelfile.*`) и шаблонных обёрток (`*.yaml.j2`, `*.toml.tmpl`) | Task 5 |
| `crates/ci-core/src/analysis/scan.rs` | `ScanOptions`, `ScanOutlier`, `subdirectories`, расчёт `suggested_scope` без загрязнения от выбросов и `include_hidden` | Task 5 |
| `crates/ci-mcp/src/pipeline.rs` | Проброс `prune` в `run_update`, детекция legacy `.agents/knowledge/` в `storage_mismatch_hint` | Task 6 |
| `crates/ci-core/src/analysis/workspace.rs` | `WorkspaceKbStatus`, `analyze_workspace_with` (явные `paths`, не-Git/missing пути, уникализация `basename`) | Task 7 |
| `src/cli/{scan,init_update,check,workspace}.rs` | Флаги CLI `--max-depth`, `--include-hidden`, `update --prune`, `workspace --repo` | Tasks 5, 6, 7 |
| `.agents/skills/context-init/SKILL.md` | Регламент штатной очистки через `prune` без прямого доступа к `.context-init/knowledge/**` | Task 6 |

---

### Task 1: Чистая L0-проекция `L0SubsystemSummary` в `context_get_subsystems`

**Files:**
- Modify: `crates/ci-mcp/src/tools.rs:40-55`
- Modify: `crates/ci-mcp/src/handlers.rs:347-401`
- Test: `crates/ci-mcp/src/lib.rs`

**Interfaces:**
- Produces: `pub struct L0SubsystemSummary { pub id: String, pub name: String, pub parent: Option<String>, pub summary: String, pub tags: Vec<String>, pub file_count: usize, pub tokens: usize, pub stale: bool, pub needs_review: bool }` в `ci_mcp::tools`.
- Consumes: `ResolvedProject::check_subsystem_staleness_with_snap`, `ci_core::analysis::subsystems::subsystem_tokens`.

- [ ] **Step 1: Написать падающий тест на размер, схему и экранирование `context_get_subsystems`**

В `crates/ci-mcp/src/lib.rs` добавить `#[tokio::test] async fn get_subsystems_fits_65_entries_and_escapes_multiline_summary()`: засеять `L0_index.json` с `65` подсистемами (у каждой по 10 элементов в `files` и `key_symbols`, а у первой — `summary` с `\n` и `"`), вызвать `server.get_subsystems(...)` и проверить:
```rust
assert!(!out.contains("truncated: true"), "{out}");
assert!(out.len() < 9_000, "bytes={}", out.len());
let json_part = out.split("\n\n[").next().unwrap();
let parsed: Vec<serde_json::Value> = serde_json::from_str(json_part).unwrap();
assert_eq!(parsed.len(), 65);
assert_eq!(parsed[0]["file_count"], 10);
assert!(parsed[0].get("files").is_none());
assert!(parsed[0].get("key_symbols").is_none());
assert!(parsed[0].get("entrypoints").is_none());
```

- [ ] **Step 2: Запустить тест и убедиться, что он падает**

Run: `rtk cargo test -p ci-mcp -- get_subsystems_fits_65_entries`  
Expected: FAIL (`truncated: true` и присутствие поля `"files"`).

- [ ] **Step 3: Объявить `L0SubsystemSummary` и перевести `ContextHandler::get_subsystems` на построчный компактный JSON**

Добавить `L0SubsystemSummary` в `crates/ci-mcp/src/tools.rs` (с `skip_serializing_if` на `parent`, `tags`, `stale`, `needs_review`). В `crates/ci-mcp/src/handlers.rs:348-400` вычислить `stale` для каждого элемента без раннего `break`, собрать `Vec<L0SubsystemSummary>`, отрендерить каждую запись через `serde_json::to_string(&entry)?` в массив `[\n  <line1>,\n  <line2>\n]` (`"[]"`, если список пуст) и передать в `paginate_content`.

- [ ] **Step 4: Проверить прохождение всех тестов `ci-mcp`**

Run: `rtk cargo test -p ci-mcp`  
Expected: PASS (включая существующий `get_subsystems_reports_live_tokens`).

- [ ] **Step 5: Коммит**

```bash
rtk git add crates/ci-mcp/src/tools.rs crates/ci-mcp/src/handlers.rs crates/ci-mcp/src/lib.rs
rtk git commit -m "feat(ci-mcp): project context_get_subsystems to compact L0 summary"
```

---

### Task 2: Бесколлизионный `sanitize_id` и дедупликация в `L0Index` / `AGENTS.md`

**Files:**
- Modify: `crates/ci-core/src/analysis/generator.rs:115-125, 1013-1020, 1045-1066`
- Modify: `crates/ci-core/src/analysis/detector.rs:1134-1138`
- Test: `crates/ci-core/src/analysis/generator.rs`, `crates/ci-core/src/analysis/detector/tests.rs`

**Interfaces:**
- Produces: бесколлизионный `pub fn sanitize_id(raw: &str) -> String` и `pub fn disambiguate_id(base_id: &str, raw_key: &str) -> String` в `ci_core::analysis::generator`.
- Consumes: `sha2::Sha256`, `crate::storage::validate_subsystem_id`.

- [ ] **Step 1: Написать падающие тесты на длинные пути, граничные разделители и коллизии `a/b_c` vs `a_b/c`**

В `generator.rs` (`mod tests`) добавить `test_sanitize_id_long_paths_hash_and_boundary_separators` и `test_normalize_index_and_agents_md_dedup_ids`:
```rust
let a = sanitize_id("infra/packages/hermes/plugins/skill-retrieval/tests/test_issue_alpha_case");
let b = sanitize_id("infra/packages/hermes/plugins/skill-retrieval/tests/test_issue_beta_case");
let sep = sanitize_id(&format!("{}____tail_exceeding_sixty_four_chars", "a".repeat(53)));
assert_ne!(a, b);
assert!(a.len() <= 64 && b.len() <= 64 && sep.len() <= 64);
assert!(!sep.contains("__"));
crate::storage::validate_subsystem_id(&a).unwrap();
crate::storage::validate_subsystem_id(&sep).unwrap();
```
В `detector/tests.rs` добавить тест `test_discover_subsystems_disambiguates_colliding_paths` с каталогами `packages/foo_bar/src` и `packages/foo/bar` (или `src/foo_bar` и `src/foo-bar`), проверяющий уникальность всех `m.id`.

- [ ] **Step 2: Запустить тесты и убедиться, что они падают**

Run: `rtk cargo test -p ci-core -- sanitize_id_long_paths`  
Expected: FAIL (`a == b`, оба обрезаны на 64 символах без хеша).

- [ ] **Step 3: Реализовать хеш-суффикс `_{8hex}` и дедупликацию**

1. В `generator.rs:sanitize_id`: если `id.chars().count() > 64`, взять первые `55` символов, вызвать `.trim_end_matches(['_', '-'])` (если пусто — `"m"`), вычислить 4 первых байта `Sha256::digest(raw.as_bytes())` в 8 hex-символов и вернуть `format!("{prefix}_{hex8}")`. Вынести этот же шаг в `pub fn disambiguate_id(base_id: &str, raw_key: &str) -> String`.
2. В `detector.rs:discover_subsystems` (после сортировки `result` по `dir_path`): найти `id`, встречающиеся `> 1` раза при разных `dir_path`, и заменить `m.id = disambiguate_id(&m.id, &m.dir_path)`.
3. В `generator.rs:normalize_index`: при совпадении `id` соседних элементов после сортировки объединять их в один, оставляя запись с не-`Heuristic` `summary_source` (при равенстве — с большим `files.len()`). В `render_root_agents_md` добавить `sorted.dedup_by(|a, b| a.id == b.id)`.

- [ ] **Step 4: Запустить тесты `ci-core`**

Run: `rtk cargo test -p ci-core`  
Expected: PASS.

- [ ] **Step 5: Коммит**

```bash
rtk git add crates/ci-core/src/analysis/generator.rs crates/ci-core/src/analysis/detector.rs crates/ci-core/src/analysis/detector/tests.rs
rtk git commit -m "fix(ci-core): prevent subsystem_id truncation collisions and deduplicate index"
```

---

### Task 3: Очистка от частного Stow-долга и префиксный обход `[scope].include`

**Files:**
- Modify: `crates/ci-core/src/analysis/scope.rs:47-140`
- Modify: `crates/ci-core/src/storage.rs:440-472`
- Modify: `crates/ci-core/src/config.rs:360-370, 614-620, 783-790, 950-960, 1080-1088, 1135-1140, 1278-1286`
- Modify: `crates/ci-core/src/analysis/generator.rs:335-345`
- Modify: `crates/ci-core/src/lib.rs:190-280`
- Test: `crates/ci-core/src/analysis/scope.rs`, `crates/ci-core/src/config.rs`

**Interfaces:**
- Produces: `pub(crate) fn include_allows_dir_traversal(rel_dir: &str, patterns: &[String]) -> bool` в `ci_core::analysis::scope`.
- Removes: `is_stow_or_dotfiles`, `L0Index::stow_packages`, `IntegrationsConfig::stow`, `CliOverrides::stow`.

- [ ] **Step 1: Написать падающие тесты на префиксный спуск `[scope].include` и тихую совместимость `stow = false`**

В `crates/ci-core/src/analysis/scope.rs` (`mod tests`) добавить `scope_include_prefix_descends_into_explicit_dot_dir_but_not_wildcard`:
- создать в temp-корне `.config/app/settings.toml`, `.agents/secret.md`, `.git/config`, `src/main.rs`;
- при `ScopeConfig { include: vec![".config/app/**".into()], ..Default::default() }` проверить `filter.files() == &[".config/app/settings.toml"]` (и отсутствие `.git/config`);
- при `ScopeConfig { include: vec!["**/*.toml".into(), "**/*.md".into()], ..Default::default() }` проверить, что ни `.config/app/settings.toml`, ни `.agents/secret.md` в выборку **не** попали.
В `crates/ci-core/src/lib.rs` заменить старые ассерты `cfg.integrations.stow` на проверку безошибочного чтения legacy TOML с `[integrations]\nstow = true`.

- [ ] **Step 2: Запустить тесты и убедиться, что они падают**

Run: `rtk cargo test -p ci-core -- scope_include_prefix`  
Expected: FAIL (`filter.files()` пуст, так как `WalkBuilder` отбросил каталог `.config`).

- [ ] **Step 3: Удалить Stow-поля и реализовать `include_allows_dir_traversal`**

1. Удалить `L0Index::stow_packages` из `storage.rs` и `generator.rs`; удалить `stow` из `IntegrationsConfig`, `CliOverrides`, `apply_env` и шаблона `config init` в `config.rs`, оставив в `PartialIntegrationsConfig` поле `#[serde(rename = "stow", default)] _legacy_stow: Option<bool>`.
2. Удалить `is_stow_or_dotfiles` из `scope.rs`. Реализовать `include_allows_dir_traversal(rel_dir: &str, patterns: &[String]) -> bool`: нормализовать паттерн (`trim_start_matches("./")`), пропустить паттерны, чей первый сегмент содержит `*`, `?`, `[` или `{`; извлечь литеральный префикс сегментов до первого метасимвола и вернуть `true`, если `rel_dir` равен литеральному префиксу, является его предком (`lit_prefix.starts_with(&format!("{rel_dir}/"))`) или потомком (`rel_dir.starts_with(&format!("{lit_prefix}/"))` при наличии `**` после префикса). Использовать функцию в `filter_entry` для всех директорий кроме `.git`.

- [ ] **Step 4: Запустить тесты `ci-core`**

Run: `rtk cargo test -p ci-core`  
Expected: PASS.

- [ ] **Step 5: Коммит**

```bash
rtk git add crates/ci-core/src/analysis/scope.rs crates/ci-core/src/storage.rs crates/ci-core/src/config.rs crates/ci-core/src/analysis/generator.rs crates/ci-core/src/lib.rs
rtk git commit -m "refactor(ci-core): remove stow-specific logic and support explicit scope.include dir traversal"
```

---

### Task 4: Универсальная гранулярность сплиттера, плоские коллекции и файлы контейнеров

**Files:**
- Modify: `crates/ci-core/src/analysis/detector.rs:865-925, 1068-1135, 1423-1515`
- Test: `crates/ci-core/src/analysis/detector/tests.rs`

**Interfaces:**
- Produces: `pub const MIN_SINGLE_FILE_SUBSYSTEM_TOKENS: usize = 1_000` и `fn is_flat_file_collection(dir: &Path) -> bool` в `ci_core::analysis::detector`.
- Consumes: `tests_classifier::is_test_path`.

- [ ] **Step 1: Написать падающие тесты на плоский `scripts/`, корневой `tests/`, порог `1 000` токенов и файлы контейнера**

В `crates/ci-core/src/analysis/detector/tests.rs` добавить 3 теста:
1. `test_flat_scripts_collection_never_splits_into_single_files`: 20 файлов `scripts/job_{01..20}.sh` по ~1 200 токенов (суммарно > 20 000) дают ровно 1 модуль `scripts` (а при добавлении `scripts/ci/` на 8 000 токенов отщепляется только подпапка `scripts_ci`).
2. `test_root_tests_dir_does_not_promote_single_test_files`: корневой `tests/` с 5 крупными `tests/e2e_*.rs` (> 18 000 токенов суммарно) остаётся единой подсистемой `tests`.
3. `test_container_keeps_direct_files_without_duplicating_child_modules`: каталог `packages/tsconfig.base.json` + `packages/ui/src/index.ts` порождает подсистемы `packages` (содержащую только `packages/tsconfig.base.json`) и `packages_ui` (содержащую `packages/ui/src/index.ts`).

- [ ] **Step 2: Запустить новые тесты и убедиться, что они падают**

Run: `rtk cargo test -p ci-core -- test_flat_scripts_collection test_root_tests_dir test_container_keeps_direct`  
Expected: FAIL (`scripts` распадается на 20 модулей, `tests` распадается на `tests_e2e_*`, `packages/tsconfig.base.json` потерян).

- [ ] **Step 3: Обновить `SplitContext::split` и обработку контейнеров в `discover_subsystems`**

1. В `detector.rs` ввести `pub const MIN_SINGLE_FILE_SUBSYSTEM_TOKENS: usize = 1_000;`.
2. Реализовать `is_flat_file_collection(dir: &Path) -> bool` по критерию из раздела 5.2 п. 3 спеки (нет пакетного манифеста в `dir`, нет `dir.join("src").is_dir()`, нет соседнего `dir.with_extension("rs").is_file()` и ни один прямой файл в `dir` не является кодовым entrypoint `main.* | lib.rs | mod.rs | index.* | app.* | server.* | __init__.py`).
3. В `SplitContext::split`:
   - удалить вторичный цикл `if has_promoted_file && !has_pinned_code_entrypoint` (`1468–1489`);
   - в первичном цикле (`1432–1451`) убрать `id != "tests"` из `is_single_test_file`, а для `entry.dir.is_none()` запретить промоушен, если `entry.tokens < MIN_SINGLE_FILE_SUBSYSTEM_TOKENS` или `is_flat_file_collection(dir)`.
4. В `discover_subsystems` (`866–925`): убрать `"profiles" | "home" | "linked" | "dot_config"` из `common_parent`. Для контейнеров с `has_subdirs == true` проверять наличие прямых релевантных файлов в `parent_dir`: если они есть — добавлять `parent_dir` в список модулей, но при сборе файлов такого контейнера пропускать поддиректории, которые сами зарегистрированы в `dirs_to_inspect`.

- [ ] **Step 4: Запустить все тесты `ci-core`**

Run: `rtk cargo test -p ci-core`  
Expected: PASS (проверить, что существующие тесты сплита крейтов в `generator.rs` и `detector/tests.rs` остаются зелёными).

- [ ] **Step 5: Коммит**

```bash
rtk git add crates/ci-core/src/analysis/detector.rs crates/ci-core/src/analysis/detector/tests.rs
rtk git commit -m "feat(ci-core): enforce 1k single-file floor, atomic flat collections and container direct files"
```

---

### Task 5: Классификатор `file_kind` и харденинг `context_scan` (`Outliers`, `max_depth`, `--include-hidden`)

**Files:**
- Create: `crates/ci-core/src/analysis/file_kind.rs`
- Modify: `crates/ci-core/src/analysis/mod.rs`
- Modify: `crates/ci-core/src/analysis/scope.rs:84-180`
- Modify: `crates/ci-core/src/analysis/scan.rs`
- Modify: `crates/ci-mcp/src/tools.rs:194-207`, `crates/ci-mcp/src/handlers.rs:580-610`, `crates/ci-mcp/src/server.rs`
- Modify: `src/cli/scan.rs`
- Test: `crates/ci-core/src/analysis/scan.rs`, `crates/ci-core/src/analysis/file_kind.rs`

**Interfaces:**
- Produces:
  - `pub fn canonical_file_ext(rel_path: &str) -> Option<String>` в `ci_core::analysis::file_kind`;
  - `pub struct ScanOptions { pub max_depth: usize, pub include_hidden: bool }`, `pub struct ScanOutlier { pub path: String, pub lines: usize, pub tokens: usize, pub share_percent: f64 }` и `pub fn scan_project_with(root: &Path, scope: &ScopeConfig, opts: &ScanOptions) -> Result<ScanReport>` в `ci_core::analysis::scan`;
  - `ScopeFilter::build_diagnostic(root: &Path, scope: &ScopeConfig, include_hidden: bool) -> Result<Self>` в `ci_core::analysis::scope`.

- [ ] **Step 1: Написать падающие тесты на `canonical_file_ext`, Context Bombs, `--include-hidden` и `clean_total_tokens == 0`**

1. В `file_kind.rs`: проверить `Dockerfile.prod` → `Some("dockerfile")`, `Modelfile.qwen2.5-ctx64k` → `Some("modelfile")`, `deploy/config.yaml.j2` → `Some("yaml.j2")`, `notes.unknown.tmpl` → `Some("tmpl")`, `LICENSE` → `None`.
2. В `scan.rs` (`mod tests`):
   - `scan_detects_outliers_excludes_context_bombs_and_keeps_code_extensions`: 2 файла `vendor_dump/a.cpp`, `vendor_dump/b.cpp` по `30_000` токенов + 10 файлов `scripts/s{0..9}.sh` по `80` токенов + `Dockerfile.v1.2` + `templates/app.toml.tmpl` (3 файла) → `report.outliers.len() == 2`, `suggested_scope.exclude` содержит `"vendor_dump/**"`, `suggested_scope.extensions` содержит `"sh"` и `"toml.tmpl"`, но не содержит `"2"` и не вытеснен `"cpp"`; передача `report.suggested_scope` обратно в `ScopeFilter::build` отсекает `vendor_dump/*` и пропускает `scripts/*.sh` и `templates/*.toml.tmpl`.
   - `scan_include_hidden_shows_dot_dirs_without_polluting_suggested_scope`: папка `.agents/guide.md` видна в `directories` при `include_hidden = true`, но не меняет `suggested_scope`.
   - `scan_all_outliers_does_not_produce_nan_shares`: репо из одного файла на `30_000` токенов сериализуется через `serde_json::to_string(&report)` без ошибки.

- [ ] **Step 2: Запустить тесты и убедиться, что они падают**

Run: `rtk cargo test -p ci-core -- file_kind scan_`  
Expected: FAIL (модуль `file_kind` и структуры `ScanOptions` / `ScanOutlier` ещё не созданы).

- [ ] **Step 3: Реализовать `file_kind.rs`, расширенный `scan_project_with` и проброс флагов в CLI/MCP**

1. Создать `crates/ci-core/src/analysis/file_kind.rs` по разделу 5.5 спеки и подключить в `analysis/mod.rs` и `ScopeFilter::build` (матчинг по `canonical_file_ext` или сырому `extension()`).
2. В `scope.rs` выделить список `ALWAYS_IGNORED_RUNTIME_DIRS` (`.git`, `.context-init`, `node_modules`, `target`, `.venv`, кэши, `.worktrees`, `.claude/worktrees`) и конструктор `ScopeFilter::build_diagnostic(root, scope, include_hidden)`, при `include_hidden = true` пропускающий скрытые dot-папки вне runtime-списка.
3. В `scan.rs` реализовать `ScanOptions` (`max_depth` зажимается в `1..=5`), `ScanOutlier`, `ScanReport { outliers, subdirectories, .. }`, расчёт `suggested_scope` по штатному `ScopeFilter` за вычетом `outliers` (при `clean_total_tokens == 0` доли равны `0.0`) и рендер `subdirectories` и `## Outliers (Context Bombs)` в `render_scan_markdown`. Оставить `scan_project(root, scope)` обёрткой с `ScanOptions::default()`.
4. Добавить `max_depth: Option<usize>` и `include_hidden: Option<bool>` в `McpScanParams`, `ContextHandler::scan`, `server.rs` и флаги `--max-depth` (`default_value_t = 2`) и `--include-hidden` в `src/cli/scan.rs`.

- [ ] **Step 4: Проверить тесты `ci-core`, `ci-mcp` и бинаря**

Run: `rtk cargo test --workspace`  
Expected: PASS.

- [ ] **Step 5: Коммит**

```bash
rtk git add crates/ci-core/src/analysis/file_kind.rs crates/ci-core/src/analysis/mod.rs crates/ci-core/src/analysis/scope.rs crates/ci-core/src/analysis/scan.rs crates/ci-mcp/src/tools.rs crates/ci-mcp/src/handlers.rs crates/ci-mcp/src/server.rs src/cli/scan.rs
rtk git commit -m "feat(scan): add file_kind classifier, outlier detection, max_depth and include_hidden"
```

---

### Task 6: Штатный прунинг `update --prune` и подсказка миграции `.agents/knowledge`

**Files:**
- Modify: `crates/ci-core/src/analysis/generator.rs:525-670`
- Modify: `crates/ci-mcp/src/pipeline.rs:30-115, 207-230`
- Modify: `crates/ci-mcp/src/tools.rs:177-193`, `crates/ci-mcp/src/handlers.rs:302-345, 620-655`, `crates/ci-mcp/src/server.rs`
- Modify: `src/cli/init_update.rs:120-185`, `src/cli/check.rs`
- Modify: `.agents/skills/context-init/SKILL.md`
- Test: `crates/ci-core/src/analysis/generator.rs`, `tests/cli.rs`

**Interfaces:**
- Produces:
  - `pub struct UpdateStats { pub added: usize, pub updated: usize, pub pruned: usize }` и `pub fn update_with_options(analysis: &ProjectAnalysis, new_subsystems: &[SubsystemInfo], knowledge_dir: &Path, agents_md: Option<StorageMode>, prune: bool) -> Result<UpdateStats>` в `ci_core::analysis::generator`;
  - `PipelineOptions::prune: bool`, `PipelineOutcome::pruned: usize` в `ci_mcp::pipeline`.

- [ ] **Step 1: Написать падающие тесты на `update_with_options(..., prune = true)` и подсказку `.agents/knowledge`**

1. В `generator.rs` (`mod tests`): тест `update_prune_removes_deleted_root_subsystem_card_and_agents_entry` — создать базу с двумя корневыми модулями (`crates/alpha`, `crates/beta`), удалить `crates/beta` с диска: при `prune = false` `beta` остаётся в индексе (`missing_in_fs == ["crates_beta"]`), при `prune = true` `stats.pruned == 1`, `subsystems/crates_beta.md` удалён, `AGENTS.md` не содержит `crates_beta`, повторный `audit` возвращает `0` дрейфа.
2. В `tests/cli.rs`: сквозной тест `cli_update_prune_cleans_missing_subsystems` и проверка сообщения `check` на репо с `.agents/knowledge/L0_index.json`.

- [ ] **Step 2: Запустить тесты и убедиться, что они падают**

Run: `rtk cargo test -p ci-core -- update_prune`  
Expected: FAIL (`update_with_options` не определена).

- [ ] **Step 3: Реализовать `prune` в генераторе, пайплайне, MCP и CLI, обновить `SKILL.md`**

1. В `generator.rs`: выделить `update_with_options(..., prune: bool) -> Result<UpdateStats>` (сохранив `update(...) -> Result<(usize, usize)>` как обёртку с `prune = false`). Собирать `matched_existing_ids: HashSet<String>` во время цикла слияния; в `index.subsystems.retain` при `prune == true` оставлять только `matched_existing_ids.contains(&s.id)`, добавляя `s.l1_path` удалённых в `superseded` и считая `pruned`.
2. В `pipeline.rs`: добавить `prune: bool` в `PipelineOptions` и `pruned: usize` в `PipelineOutcome`; в `storage_mismatch_hint` добавить проверку `project.root.join(".agents/knowledge/L0_index.json").is_file()` с понятной подсказкой переноса базы в `.context-init/knowledge/`.
3. Пробросить `prune` в `McpUpdateParams`, `ContextHandler::update` (включить `pruned` в текст ответа), `UpdateArgs` (`#[arg(long)] pub prune: bool`). В `ContextHandler::check_drift` и `src/cli/check.rs` при `!missing_in_fs.is_empty()` добавлять подсказку про `update --prune` / `prune=true`.
4. В `.agents/skills/context-init/SKILL.md` добавить раздел штатной очистки удалённых подсистем через `context_update(prune=true)` / `rtk context-init update . --prune --compact`.

- [ ] **Step 4: Запустить тесты по всему workspace**

Run: `rtk cargo test --workspace`  
Expected: PASS.

- [ ] **Step 5: Коммит**

```bash
rtk git add crates/ci-core/src/analysis/generator.rs crates/ci-mcp/src/pipeline.rs crates/ci-mcp/src/tools.rs crates/ci-mcp/src/handlers.rs crates/ci-mcp/src/server.rs src/cli/init_update.rs src/cli/check.rs .agents/skills/context-init/SKILL.md tests/cli.rs
rtk git commit -m "feat(update): add --prune flag to clean deleted root subsystems and detect legacy .agents/knowledge"
```

---

### Task 7: Пакетный `context_workspace` (`paths` + `kb_status`) и финальная самопроверка

**Files:**
- Modify: `crates/ci-core/src/analysis/workspace.rs`
- Modify: `crates/ci-mcp/src/tools.rs:208-218`, `crates/ci-mcp/src/handlers.rs:590-615`, `crates/ci-mcp/src/server.rs`
- Modify: `src/cli/workspace.rs`
- Test: `crates/ci-core/src/analysis/workspace.rs`, `crates/ci-mcp/src/lib.rs`

**Interfaces:**
- Produces:
  - `pub enum WorkspaceKbStatus { Ok { subsystems: usize }, Drift { subsystems: usize, missing: usize, untracked: usize, git_stale: bool }, LegacyAgentsDir, Uninitialized, MissingPath }` и `pub fn analyze_workspace_with(workspace_root: &Path, explicit_paths: Option<&[PathBuf]>, git: &dyn GitBackend, inspect_kb: &(dyn Fn(&Path) -> WorkspaceKbStatus + Sync)) -> Result<WorkspaceReport>` в `ci_core::analysis::workspace`;
  - обновлённый `ContextHandler::workspace_with(root: &Path, paths: Option<&[PathBuf]>, max_tokens: usize, cursor: Option<&str>) -> Result<String>`.

- [ ] **Step 1: Написать падающие тесты на `paths`, `WorkspaceKbStatus` и коллизии `basename`**

В `crates/ci-mcp/src/lib.rs` добавить `#[tokio::test] async fn workspace_explicit_paths_reports_kb_status_and_disambiguates_names()`:
- создать 4 пути: `/tmp/.../a/app` (инициализирована база, `Ok`), `/tmp/.../b/app` (база с удалённым модулем, `Drift`), `/tmp/.../plain_dir` (существующая не-Git папка без базы, `Uninitialized`, `branch == "-"`), `/tmp/.../missing_dir` (не существует, `MissingPath`);
- вызвать `server.workspace(Parameters(McpWorkspaceParams { paths: Some(...), .. }))` и проверить присутствие уникальных имён `a/app`, `b/app` и всех 4 статусов в таблице без возврата ошибки MCP.

- [ ] **Step 2: Запустить тест и убедиться, что он падает**

Run: `rtk cargo test -p ci-mcp -- workspace_explicit_paths`  
Expected: FAIL (поле `paths` и `WorkspaceKbStatus` отсутствуют).

- [ ] **Step 3: Реализовать `WorkspaceKbStatus`, `analyze_workspace_with` и проброс в MCP/CLI**

1. В `workspace.rs`: добавить `WorkspaceKbStatus` и поля `path: String`, `kb_status: WorkspaceKbStatus` в `WorkspaceRepo`. Реализовать разрешение `explicit_paths` (дедупликация существующих путей по `canonicalize`, обработка несуществующих как `MissingPath` и не-Git папок с `branch = "-"` без ошибки) и уникализацию совпадающих `basename` добавлением родительского сегмента (`parent/name`), используемого и в `CrossLink.repos`. Обновить таблицу в `render_workspace_l0` (`| Repository | Branch | Dirty | KB Status | Subsystems |`). Сохранить `analyze_workspace(root, git)` как обёртку над `analyze_workspace_with`.
2. В `ci-mcp`: реализовать инспектор `kb_status` через `ResolvedProject::load` + проверку `.agents/knowledge/L0_index.json` + `pipeline::run_check`, пробросить `paths: Option<Vec<String>>` из `McpWorkspaceParams` и `--repo <PATH>` из `src/cli/workspace.rs`.

- [ ] **Step 4: Полная проверка кода, обновление собственной базы `context-init` и аудит дрейфа**

Run:
```bash
rtk cargo fmt --all -- --check
rtk cargo clippy --workspace --all-targets -- -D warnings
rtk cargo test --workspace
rtk cargo run -- update . --prune --compact
rtk cargo run -- check . --compact --exit-code
```
Expected: все тесты и `clippy` проходят с `0` предупреждений; `check` выводит `OK:` (`0 drift`), сиротская `tests_regression_benchmark` и микроподсистема `crates_ci-core_analysis_detector_mise` удалены из `.context-init/knowledge/` и `AGENTS.md`.

- [ ] **Step 5: Финальный коммит**

```bash
rtk git add crates/ci-core/src/analysis/workspace.rs crates/ci-mcp/src/tools.rs crates/ci-mcp/src/handlers.rs crates/ci-mcp/src/server.rs crates/ci-mcp/src/lib.rs src/cli/workspace.rs .context-init/knowledge AGENTS.md
rtk git commit -m "feat(workspace): add explicit paths and kb_status summary and refresh repo knowledge base"
```
