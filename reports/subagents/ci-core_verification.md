# Отчет о независимой верификации Шага 2 вехи v0.3 «Фундамент» (Крейт `crates/ci-core`)

**Дата аудита:** 2026-10-19  
**Аудитор:** DeepInvestigator (Independent Adversarial Review & Verification)  
**Итоговый статус:** **PASS (все 7 пунктов задания и 14 критериев верификации выполнены; выявлено 5 неблокирующих граничных замечаний F1–F5 для учета на следующих шагах)**

---

## 1. Сводная таблица верификации

| # | Пункт чек-листа | Нормативный документ | Статус | Доказательство |
| :--- | :--- | :--- | :--- | :--- |
| **1.1** | Разрешение кроссплатформенных путей (`$XDG_CONFIG_HOME`, `~/.config/context-init/config.toml`, `%APPDATA%` fallback, `<repo>/.context-init.toml`, `~/.local/share/context-init/projects/<project-id>/`, `~/.cache/context-init/`) | [milestone_decisions.md](file:///home/gidragir/projects/context-init/milestone_decisions.md#L50) (D10), [orchestration_architecture.md](file:///home/gidragir/.gemini/antigravity/brain/33f3fc15-fa35-49db-85ed-27e725f95875/orchestration_architecture.md#L39-L49) | **PASS** | [`StoragePaths`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L24-L171), тест [`storage_paths_xdg_and_defaults_and_appdata_fallback`](file:///home/gidragir/projects/context-init/crates/ci-core/src/lib.rs#L91-L172) *(см. замечания F2, F3)* |
| **1.2** | Каскад приоритетов: `CLI > ENV (CONTEXT_INIT_*) > <repo>/.context-init.toml > глобальный config.toml > defaults` | [milestone_decisions.md](file:///home/gidragir/projects/context-init/milestone_decisions.md#L51) (D11) | **PASS** | [`Config::from_layers`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L754-L779), [`apply_env`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L870-L946), [`apply_cli_overrides`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L949-L984), тест [`config_priority_cascade_and_overrides`](file:///home/gidragir/projects/context-init/crates/ci-core/src/lib.rs#L175-L282) |
| **1.3** | Поддержка `[integrations]` (`rtk = "auto"\|"on"\|"off"`, `mise`, `stow`, детекция с `.exe`), `language`, `[models.<alias>]`, `[roles]` (`init`, `update`, `task`, `search`, `"none"`), `[telemetry]` (`enabled = false`), `max_response_tokens = 4000`, `touch_agents_md = false` | D1, D12, D14–D16, D19, D28 | **PASS** | [`RtkMode`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L176-L227), [`is_binary_available_in_dirs`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L244-L273), [`ModelKind`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L324-L380), [`RolesConfig`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L461-L528), [`Config`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L615-L654) |
| **1.4** | Поддержка операций `init\|show\|path\|validate` для конфига | [milestone_decisions.md](file:///home/gidragir/projects/context-init/milestone_decisions.md#L51) (D11) | **PASS** | [`init_config_file`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L1059-L1074), [`show_toml`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L1049-L1051), [`format_paths`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L147-L170), [`validate`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L991-L1046) |
| **2.1** | Вычисление `project-id` (нормализованный git remote без кредов/протокола/`.git` + хэш пути) и SHA-256 `hash_project_id` | D1, D28 | **PASS** | [`normalize_git_remote`](file:///home/gidragir/projects/context-init/crates/ci-core/src/storage.rs#L90-L150), [`compute_project_id`](file:///home/gidragir/projects/context-init/crates/ci-core/src/storage.rs#L154-L183), [`hash_project_id`](file:///home/gidragir/projects/context-init/crates/ci-core/src/storage.rs#L186-L190), тест [`project_id_normalization_and_sha256_anonymization`](file:///home/gidragir/projects/context-init/crates/ci-core/src/lib.rs#L388-L417) *(см. замечание F1)* |
| **2.2** | Парсинг/сериализация OKF Sidecar (`directory`, `source`, `generated_by`, `freshness` + Markdown body) и `extract_l0_abstract` (первый параграф после `# H1`, до 256 символов) | [references/openviking_spec.md](file:///home/gidragir/projects/context-init/references/openviking_spec.md#L18-L48) (§1–2) | **PASS** | [`CardFrontmatter`](file:///home/gidragir/projects/context-init/crates/ci-core/src/openviking.rs#L70-L277), [`OkfCard`](file:///home/gidragir/projects/context-init/crates/ci-core/src/openviking.rs#L281-L345), [`extract_l0_abstract_with_limit`](file:///home/gidragir/projects/context-init/crates/ci-core/src/openviking.rs#L353-L409) *(см. замечание F4)* |
| **2.3** | `L0Index` со `schema_version`, чтение старого формата `.agents/knowledge/L0_index.json` и миграция `migrate_in_repo_knowledge` | [milestone_decisions.md](file:///home/gidragir/projects/context-init/milestone_decisions.md#L78) (D23) | **PASS** | [`L0Index`](file:///home/gidragir/projects/context-init/crates/ci-core/src/storage.rs#L295-L405), [`migrate_in_repo_knowledge_with_options`](file:///home/gidragir/projects/context-init/crates/ci-core/src/storage.rs#L444-L516), тест [`legacy_l0_index_compatibility_and_migrate_in_repo_knowledge`](file:///home/gidragir/projects/context-init/crates/ci-core/src/lib.rs#L455-L528) |
| **2.4** | Нормализация путей к `/` и устойчивость к CRLF | [milestone_decisions.md](file:///home/gidragir/projects/context-init/milestone_decisions.md#L119) (разд. 3) | **PASS** | [`normalize_path_str`](file:///home/gidragir/projects/context-init/crates/ci-core/src/storage.rs#L31-L78), [`normalize_line_endings`](file:///home/gidragir/projects/context-init/crates/ci-core/src/storage.rs#L81-L86) |
| **3.1** | Трейт `GitBackend`, `SystemGitBackend` (`-z` / `--porcelain=v1 -z`) и `check_card_staleness` (`stale` + `diff --stat`) | [milestone_decisions.md](file:///home/gidragir/projects/context-init/milestone_decisions.md#L27-L28) (D2, D3), [openviking_spec.md](file:///home/gidragir/projects/context-init/references/openviking_spec.md#L57-L60) (§3.3) | **PASS** | [`GitBackend`](file:///home/gidragir/projects/context-init/crates/ci-core/src/git.rs#L53-L83), [`SystemGitBackend`](file:///home/gidragir/projects/context-init/crates/ci-core/src/git.rs#L87-L289), [`check_frontmatter_staleness`](file:///home/gidragir/projects/context-init/crates/ci-core/src/git.rs#L355-L453), тесты [`system_git_backend_and_card_staleness_on_temp_and_current_repo`](file:///home/gidragir/projects/context-init/crates/ci-core/src/lib.rs#L531-L680) и [`regression_git_staleness_empty_blob_detached_head_and_unborn_branch`](file:///home/gidragir/projects/context-init/crates/ci-core/src/lib.rs#L978-L1045) *(см. замечание F5)* |
| **4.1** | Трейт `TelemetrySink`, `JsonlTelemetrySink` (`YYYY-MM.jsonl` со `schema_version` строго при `enabled = true`), сигналы D29 без путей/кода/имен объектов, `render_telemetry_report` со скрытием `note` по умолчанию | [milestone_decisions.md](file:///home/gidragir/projects/context-init/milestone_decisions.md#L93-L95) (D28–D30, O6) | **PASS** | [`TelemetrySignal`](file:///home/gidragir/projects/context-init/crates/ci-core/src/telemetry.rs#L103-L110), [`JsonlTelemetrySink`](file:///home/gidragir/projects/context-init/crates/ci-core/src/telemetry.rs#L174-L287), [`render_telemetry_report`](file:///home/gidragir/projects/context-init/crates/ci-core/src/telemetry.rs#L315-L522), тест [`telemetry_sink_opt_in_jsonl_and_note_redaction`](file:///home/gidragir/projects/context-init/crates/ci-core/src/lib.rs#L683-L747) |
| **5.1** | Отсутствие упоминаний «1С» / «1C» в `Cargo.toml`, `crates/`, `src/` | [milestone_decisions.md](file:///home/gidragir/projects/context-init/milestone_decisions.md#L117) (разд. 3) | **PASS** | `rtk rg -i '1[cс]' Cargo.toml crates/ src/` → exit code `1` (0 совпадений) |
| **6.1** | `rtk cargo check -p ci-core` и `rtk cargo check --workspace` | Объективная проверка | **PASS** | Код возврата `0` (`0.10s` / `0.06s`) |
| **6.2** | `rtk cargo clippy -p ci-core --all-targets -- -D warnings` и по всему `--workspace` | Объективная проверка (D26) | **PASS** | Код возврата `0` (`No issues found`) |
| **6.3** | `rtk cargo nextest run -p ci-core` и по всему `--workspace` | Объективная проверка (D26) | **PASS** | Код возврата `0` (`15 passed` в `ci-core`, `53 passed` по всему `--workspace`) |

---

## 2. Детальный технический анализ по подсистемам

### Пункт 1: Модуль `config` ([crates/ci-core/src/config.rs](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs)) — `PASS`

1. **Иерархия путей (D10, [orchestration_architecture.md](file:///home/gidragir/.gemini/antigravity/brain/33f3fc15-fa35-49db-85ed-27e725f95875/orchestration_architecture.md#L39-L49)):**
   - Структура [`StoragePaths`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L24-L42) и метод [`StoragePaths::resolve_with_env`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L52-L126) разрешают:
     - `global_config_path`: `$CONTEXT_INIT_CONFIG` > `$XDG_CONFIG_HOME/context-init/config.toml` > `~/.config/context-init/config.toml`.
     - `fallback_global_config_path`: `%APPDATA%/context-init/config.toml` (или `dirs_next::config_dir()` на Windows); метод [`effective_global_config_path`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L135-L144) переключается на `%APPDATA%`, если основной файл отсутствует, а fallback существует.
     - `project_config_path`: `<repo>/.context-init.toml`.
     - `data_dir` и `projects_dir`: `$XDG_DATA_HOME/context-init` (иначе `~/.local/share/context-init`), метод [`project_storage_dir`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L130-L132) возвращает `<data_dir>/projects/<project-id>`.
     - `cache_dir`: `$XDG_CACHE_HOME/context-init` (иначе `~/.cache/context-init`).
     - Пустые и состоящие из пробелов значения `XDG_*` корректно отфильтровываются ([config.rs:60-65](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L60-L65)).

2. **Каскад приоритетов (D11):**
   - В [`Config::from_layers`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L754-L779) слои накладываются строго по возрастанию приоритета:
     1. `Config::default()` (`language = "ru"`, `max_response_tokens = 4000`, `touch_agents_md = false`, `rtk = "auto"`, `roles.* = ["none"]`, `telemetry.enabled = false`).
     2. Глобальный TOML (`apply_toml_str`).
     3. Проектный `.context-init.toml` (`apply_toml_str` с частичным переопределением полей существующих `[models.<alias>]`).
     4. Переменные окружения `CONTEXT_INIT_*` (`apply_env`: `CONTEXT_INIT_LANGUAGE`, `CONTEXT_INIT_MAX_RESPONSE_TOKENS`, `CONTEXT_INIT_TOUCH_AGENTS_MD`, `CONTEXT_INIT_RTK`, `CONTEXT_INIT_MISE`, `CONTEXT_INIT_STOW`, `CONTEXT_INIT_TELEMETRY_ENABLED`, `CONTEXT_INIT_MODEL`, `CONTEXT_INIT_ROLE_{INIT,UPDATE,TASK,SEARCH}`).
     5. Флаги командной строки [`CliOverrides`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L540-L603) (`apply_cli_overrides`), где `--model <alias>` применяется ко всем ролям, а `--role <role>=<alias[,alias2]>` точечно переопределяет конкретную роль.
   - Все промежуточные десериализаторы TOML (`PartialConfig`, `PartialModelConfig`, `PartialRolesConfig`, `PartialIntegrationsConfig`, `PartialTelemetryConfig`, строки [config.rs:656-701](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L656-L701)) защищены атрибутом `#[serde(deny_unknown_fields)]`, что гарантирует отлов опечаток в ключах TOML.

3. **Интеграции, модели, роли и операции `init|show|path|validate` (D11, D12, D14–D16, D19, D28):**
   - Детекция бинарников [`is_binary_available_in_dirs`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L244-L273) проверяет как `<bin>`, так и `<bin>.exe` на всех платформах.
   - Перечисление [`ModelKind`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L324-L339) включает все 3 MVP-вида (`ollama`, `command`, `agent` по D14) и все 4 точки расширения (`openai_compat`, `anthropic`, `gemini`, `sampling` по D15). Поле `timeout_ms` поддерживает алиас `timeout` (`#[serde(alias = "timeout")]`).
   - Поддержаны все 4 операции управления конфигом из D11:
     - `init`: [`Config::init_config_file`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L1059-L1074) и [`generate_template_toml`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L1083-L1111).
     - `show`: [`Config::show_toml`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L1049-L1051).
     - `path`: [`StoragePaths::format_paths`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L147-L170).
     - `validate`: [`Config::validate`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L991-L1046) проверяет `max_response_tokens > 0`, непустой `language`, наличие непустого `command` у моделей `kind = "command"`, и существование каждого алиаса из цепочек `[roles]` в `[models]` (либо равенство `"none"`).

---

### Пункт 2: Модули `storage` и `openviking` ([crates/ci-core/src/storage.rs](file:///home/gidragir/projects/context-init/crates/ci-core/src/storage.rs), [crates/ci-core/src/openviking.rs](file:///home/gidragir/projects/context-init/crates/ci-core/src/openviking.rs)) — `PASS`

1. **Идентификация проекта и анонимизация (D1, D28):**
   - [`normalize_git_remote`](file:///home/gidragir/projects/context-init/crates/ci-core/src/storage.rs#L90-L150) удаляет схемы (`https://`, `http://`, `ssh://`, `git://`, `file://`), учетные данные `user:token@` в authority-части (не затрагивая `@` в имени репозитория в SCP-формате `git@host:pkg@2.git`), нестандартные порты (`:2222`), хвостовые слэши и суффикс `.git`, приводя результат к нижнему регистру.
   - [`compute_project_id`](file:///home/gidragir/projects/context-init/crates/ci-core/src/storage.rs#L154-L183) формирует детерминированный идентификатор `<remote_or_dir_slug>-<8_hex_sha256_of_canonical_path>`.
   - [`hash_project_id`](file:///home/gidragir/projects/context-init/crates/ci-core/src/storage.rs#L186-L190) возвращает полный 64-символьный SHA-256 hex-дайджест для телеметрии.

2. **Формат карточек OpenViking OKF Sidecar ([references/openviking_spec.md](file:///home/gidragir/projects/context-init/references/openviking_spec.md#L18-L48) §2):**
   - В [`crates/ci-core/src/openviking.rs`](file:///home/gidragir/projects/context-init/crates/ci-core/src/openviking.rs) реализованы структуры `OkfCard`, `CardFrontmatter`, `CardSource` (`kind`, `blob_sha`, `commit_sha`), `CardGeneratedBy` (`component`, `role`, `model`), `CardFreshness` (`total_entries`, `sampled_entries`, `unsampled_entries`, `pending_child_changes`).
   - Парсер [`CardFrontmatter::parse_yaml`](file:///home/gidragir/projects/context-init/crates/ci-core/src/openviking.rs#L102-L276) устойчив к CRLF, UTF-8 BOM, табам, кавычкам, строчным комментариям `#` и flow-style словарям (`source: { kind: git, ... }`).
   - Функция [`extract_l0_abstract_with_limit`](file:///home/gidragir/projects/context-init/crates/ci-core/src/openviking.rs#L353-L409) извлекает первый параграф после заголовка `# H1` (останавливаясь перед подзаголовками `##`, списками `- ` / `* ` и блоками кода) и усекает его до `L0_ABSTRACT_MAX_CHARS = 256` символов с многоточием `...`.

3. **Схема `L0Index` v2, обратная совместимость и миграция (D23):**
   - [`L0Index`](file:///home/gidragir/projects/context-init/crates/ci-core/src/storage.rs#L295-L321) содержит поле `schema_version: u32`. При чтении старого `.agents/knowledge/L0_index.json` без поля `schema_version` выставляется `LEGACY_L0_SCHEMA_VERSION = 1` (`is_legacy() == true`), а при создании нового или вызове `upgrade_to_current()` — `CURRENT_L0_SCHEMA_VERSION = 2` с нормализацией всех путей к `/`.
   - [`migrate_in_repo_knowledge`](file:///home/gidragir/projects/context-init/crates/ci-core/src/storage.rs#L436-L516) рекурсивно переносит `.agents/knowledge` в центральное хранилище проекта, обновляет `L0_index.json` до версии `2`, удаляет `.agents/knowledge` и сохраняет соседние каталоги (например, `.agents/skills`).

---

### Пункт 3: Модуль `git` ([crates/ci-core/src/git.rs](file:///home/gidragir/projects/context-init/crates/ci-core/src/git.rs)) — `PASS`

- Трейт [`GitBackend`](file:///home/gidragir/projects/context-init/crates/ci-core/src/git.rs#L53-L83) определяет методы `head_commit`, `remote_url`, `blob_sha`, `status_porcelain_z`, `diff_name_only`, `diff_stat`.
- [`SystemGitBackend`](file:///home/gidragir/projects/context-init/crates/ci-core/src/git.rs#L87-L289) вызывает системный `git` с `-c core.quotePath=false`, использует NUL-разделители (`status --porcelain=v1 -z`, `diff --name-only -z`) и парсеры [`parse_porcelain_z`](file:///home/gidragir/projects/context-init/crates/ci-core/src/git.rs#L292-L333) / [`parse_nul_paths`](file:///home/gidragir/projects/context-init/crates/ci-core/src/git.rs#L336-L343), корректно обрабатывающие пробелы, переименования (`R`) и копирования (`C`).
- Функция [`check_card_staleness`](file:///home/gidragir/projects/context-init/crates/ci-core/src/git.rs#L346-L453) сравнивает `commit_sha` и `blob_sha` карточки с текущим состоянием репозитория, учитывает как коммиты (`diff_name_only`), так и незакоммиченные изменения рабочего дерева (`status_porcelain_z`), корректно помечает карточку с пустым `recorded_blob_sha` как `stale = true` и возвращает компактный `diff --stat`.

---

### Пункт 4: Модуль `telemetry` ([crates/ci-core/src/telemetry.rs](file:///home/gidragir/projects/context-init/crates/ci-core/src/telemetry.rs)) — `PASS`

- Реализованы все 5 категорий сигналов D29 без путей к файлам, исходного кода и имен объектов: [`OperationalSignal`](file:///home/gidragir/projects/context-init/crates/ci-core/src/telemetry.rs#L19-L38), [`EfficiencySignal`](file:///home/gidragir/projects/context-init/crates/ci-core/src/telemetry.rs#L42-L57), [`LlmSignal`](file:///home/gidragir/projects/context-init/crates/ci-core/src/telemetry.rs#L61-L78), [`SessionPatternSignal`](file:///home/gidragir/projects/context-init/crates/ci-core/src/telemetry.rs#L82-L87), [`FeedbackSignal`](file:///home/gidragir/projects/context-init/crates/ci-core/src/telemetry.rs#L91-L99).
- [`JsonlTelemetrySink`](file:///home/gidragir/projects/context-init/crates/ci-core/src/telemetry.rs#L174-L287) реализует трейт [`TelemetrySink`](file:///home/gidragir/projects/context-init/crates/ci-core/src/telemetry.rs#L163-L169): при `enabled == false` метод `record` является строгим no-op (не создает директорий и файлов), а при `enabled == true` дописывает события со `schema_version = 1` и SHA-256 хэшем `project_id_hash` в `<telemetry_dir>/YYYY-MM.jsonl`.
- [`render_telemetry_report(events, include_notes)`](file:///home/gidragir/projects/context-init/crates/ci-core/src/telemetry.rs#L315-L522) формирует Markdown-дайджест с YAML frontmatter и по умолчанию (`include_notes = false`) скрывает свободный текст `note` из `FeedbackSignal` во избежание утечки чувствительных имен до ручного ревью (уточнение O6).

---

### Пункты 5 и 6: Проверка инвариантов и объективные проверки сборки/линтера/тестов — `PASS`

1. **Отсутствие запрещенных доменных упоминаний:**
   ```bash
   rtk rg -i '1[cс]' Cargo.toml crates/ src/
   # Exit code: 1 (0 matches)
   ```
2. **Проверка компиляции (`cargo check`):**
   ```text
   $ rtk cargo check -p ci-core && rtk cargo check --workspace
   Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.10s
   Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
   Exit code: 0
   ```
3. **Проверка статического анализатора (`cargo clippy`):**
   ```text
   $ rtk cargo clippy -p ci-core --all-targets -- -D warnings && rtk cargo clippy --workspace --all-targets -- -D warnings
   cargo clippy: No issues found
   cargo clippy: No issues found
   Exit code: 0
   ```
4. **Запуск тестового набора (`cargo nextest`):**
   ```text
   $ rtk cargo nextest run -p ci-core && rtk cargo nextest run --workspace
   cargo nextest: 15 passed (1 binary, 0.032s)
   cargo nextest: 53 passed (6 binaries, 0.224s)
   Exit code: 0
   ```
   Все 15 unit-тестов в [`crates/ci-core/src/lib.rs`](file:///home/gidragir/projects/context-init/crates/ci-core/src/lib.rs#L84-L1100) (в отчете разработчика `ci-core_summary.md` в тексте было указано 11, фактически после добавления регрессионных тестов их 15) проходят за ~32 мс.

---

## 3. Выявленные граничные замечания (Adversarial Audit Findings)

При углубленном анализе исходного кода `crates/ci-core` выявлены 5 неблокирующих граничных случаев, пропущенных при первичном ревью:

1. **F1 — `normalize_git_remote` удаляет числовой namespace владельца в SCP-ссылках без схемы ([storage.rs:97-134](file:///home/gidragir/projects/context-init/crates/ci-core/src/storage.rs#L97-L134)):**
   - Флаг `had_scheme` вычисляется на строках 97–104 и используется на строке 107, но **не проверяется** на строках 123–125:
     ```rust
     if !between.is_empty() && between.chars().all(|c| c.is_ascii_digit()) {
         // `host:2222/owner/repo` -> strip `:2222`
         s = format!("{}{}", &s[..colon_pos], &after_colon[rel_slash..]);
     ```
   - В SCP-синтаксисе Git (`git@github.com:12345/repo.git`, где `had_scheme == false`) сегмент после `:` всегда является путем репозитория, а не портом. Если имя пользователя или группы состоит только из цифр (`12345`), `normalize_git_remote("git@github.com:12345/repo.git")` ошибочно принимает `12345` за номер порта и возвращает `"github.com/repo"`, тогда как для `"https://github.com/12345/repo.git"` возвращает `"github.com/12345/repo"`. Проверка должна иметь вид `if had_scheme && !between.is_empty() && between.chars().all(|c| c.is_ascii_digit())`.

2. **F2 — `StoragePaths::resolve` и `Config::load` затеняют проверку `HOME`/`USERPROFILE` внутри `resolve_with_env` ([config.rs:46-49, 67-72, 706-709](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L46-L72)):**
   - В [`StoragePaths::resolve`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L46-L49) и [`Config::load`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L706-L709) вызывается `let home = dirs_next::home_dir();` и передается как `home.as_deref()` в `resolve_with_env`.
   - Поскольку в [`resolve_with_env`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L67-L72) аргумент `home_dir` имеет приоритет над `non_empty_env("HOME")` и `non_empty_env("USERPROFILE")`, а в конце цепочки уже стоит `.or_else(dirs_next::home_dir)`, при вызове `StoragePaths::resolve` / `Config::load` ветки `non_empty_env("HOME")` и `non_empty_env("USERPROFILE")` оказываются недостижимы, когда `dirs_next::home_dir()` возвращает `Some(...)` (включая Windows, где `dirs_next::home_dir()` использует `FOLDERID_Profile`, игнорируя `$HOME`, и Unix при `HOME=""`, где `dirs_next::home_dir()` возвращает пустой `PathBuf`).

3. **F3 — `effective_global_config_path` может откатываться на `%APPDATA%` даже при явно заданном `CONTEXT_INIT_CONFIG` или `XDG_CONFIG_HOME` ([config.rs:84-95, 135-144](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L84-L95)):**
   - Поле `fallback_global_config_path` (`%APPDATA%\context-init\config.toml`) заполняется даже тогда, когда пользователь явно задал `CONTEXT_INIT_CONFIG` или `XDG_CONFIG_HOME`. Если файл по явно заданному пути `CONTEXT_INIT_CONFIG` еще не создан, а в `%APPDATA%\context-init\config.toml` файл существует, [`effective_global_config_path()`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L135-L144) вернет путь из `%APPDATA%` вместо явно запрошенного `CONTEXT_INIT_CONFIG`.

4. **F4 — `CardFrontmatter::to_yaml` сериализует строковые поля без кавычек при наличии последовательности ` #` ([openviking.rs:83-99, 451-468](file:///home/gidragir/projects/context-init/crates/ci-core/src/openviking.rs#L83-L99)):**
   - Парсер [`strip_yaml_inline_comment`](file:///home/gidragir/projects/context-init/crates/ci-core/src/openviking.rs#L451-L468) отрезает любой символ `#`, перед которым стоит пробел вне кавычек. Однако [`CardFrontmatter::to_yaml`](file:///home/gidragir/projects/context-init/crates/ci-core/src/openviking.rs#L83-L99) выводит `directory`, `role` и `model` без кавычек. Если `directory` или `model` содержит пробел перед `#` (например, `"src/dir #1"`), после `to_markdown()` и повторного `OkfCard::parse()` часть `" #1"` будет отброшена как YAML-комментарий.

5. **F5 — `check_frontmatter_staleness` молча игнорирует ошибки `status_porcelain_z` ([git.rs:385](file:///home/gidragir/projects/context-init/crates/ci-core/src/git.rs#L385)):**
   - При ошибке `diff_name_only` на строках [git.rs:371-381](file:///home/gidragir/projects/context-init/crates/ci-core/src/git.rs#L371-L381) выставляется `commit_lookup_failed = true`, что переводит карточку в `stale = true`. Напротив, на строке [git.rs:385](file:///home/gidragir/projects/context-init/crates/ci-core/src/git.rs#L385) используется `if let Ok(status_entries) = backend.status_porcelain_z(repo_root)`, поэтому сбой `git status` игнорируется и при `HEAD == recorded_commit` незакоммиченные изменения внутри директории могут не пометить карточку как `stale`.

---

## 4. Remaining Questions & Gaps

- **Интеграция `ci-core` в `ci-llm`, `ci-mcp` и корневой бинарник `context-init`:** В рамках Шага 2 полностью реализован и протестирован фундамент `crates/ci-core` (включая закрытие замечания Шага 1 по `.workspace = true` и `version = "0.3.0"` в `[workspace.dependencies]`). При этом корневой бинарник (`src/main.rs`, `src/types.rs`, `src/service.rs`) и крейты-заглушки `crates/ci-llm` / `crates/ci-mcp` пока не переключены на использование типов из `ci-core` — это предмет следующих шагов вехи v0.3.
- **Учет замечаний F1–F5 при интеграции CLI и MCP:** При реализации команд `context-init config`, `context-init migrate` и stale-валидации в `ci-mcp` рекомендуется учесть граничные случаи F1–F5 (в частности, проверку `had_scheme` в `normalize_git_remote` и передачу `None` вместо `dirs_next::home_dir()` в `StoragePaths::resolve` / `Config::load`).
- **Окружение субагента и `sccache` в `PATH`:** В глобальном `~/.cargo/config.toml` хоста настроен `rustc-wrapper = "sccache"`. В минимальном неинтерактивном shell-окружении субагента каталог с `sccache` и `cargo-nextest` находится в `$__MISE_ZSH_ACTIVATE_PATH` (`~/.local/share/mise/shims`), поэтому при запуске `cargo` из субагентов необходимо экспортировать `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH"`.
