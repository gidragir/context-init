# Шаг 2 вехи v0.3 «Фундамент»: Крейт `crates/ci-core`

## 1. Сводка изменений

| Файл | Действие | Описание |
| :--- | :--- | :--- |
| `Cargo.toml` | Изменен | В `[workspace.dependencies]` добавлен `version = "0.3.0"` для `ci-core`, `ci-llm`, `ci-mcp`, а в `[dependencies]` корневого пакета подключено наследование `.workspace = true` (закрытие техдолга O2 / R5). |
| `crates/ci-llm/Cargo.toml` | Изменен | Переведена зависимость `ci-core` на `ci-core.workspace = true` (O2 / R5). |
| `crates/ci-mcp/Cargo.toml` | Изменен | Переведены зависимости `ci-core` и `ci-llm` на `.workspace = true` (O2 / R5). |
| `crates/ci-core/src/config.rs` | Создан | Реализован модуль `config` (D10–D12, D16, D19, D28): `StoragePaths` (`$XDG_CONFIG_HOME`, `$XDG_DATA_HOME`, `$XDG_CACHE_HOME`, `%APPDATA%` fallback), каскад приоритетов `CliOverrides > ENV (CONTEXT_INIT_*) > <repo>/.context-init.toml > глобальный config.toml > defaults`, секции `[integrations]` (`RtkMode` + кроссплатформенная детекция бинарников с `.exe`), `[models.<alias>]` (`ModelKind`: `ollama`, `command`, `agent`, `openai_compat`, `anthropic`, `gemini`, `sampling`; поддержка алиаса `timeout` и `deny_unknown_fields`), `[roles]` (`init`, `update`, `task`, `search` с `--model` и `--role <role>=<alias>`), `[telemetry]`, а также генерация шаблона TOML и валидация конфига (включая проверку непустого `command` для `kind = "command"`). |
| `crates/ci-core/src/openviking.rs` | Создан | Реализованы структуры и парсер/сериализатор карточек OpenViking OKF Sidecar (`references/openviking_spec.md` §2): `OkfCard`, `CardFrontmatter`, `CardSource`, `CardGeneratedBy`, `CardFreshness`, CRLF-устойчивый разбор YAML frontmatter `--- ... ---` (с поддержкой inline-комментариев `#` и flow-style словарей `{ ... }`) + Markdown-тела и извлечение L0-абстракта (`extract_l0_abstract`, первый параграф после `# H1` до 256 символов). |
| `crates/ci-core/src/storage.rs` | Создан | Реализован модуль `storage` (D1, D2, D23, D28): `normalize_path` (`/`, сворачивание `/./` и `/.`), `normalize_line_endings`, `normalize_git_remote` (без кредов и `.git`, с корректным отделением SCP authority до `:`), `compute_project_id`, `hash_project_id` (SHA-256), `L0Index` со `schema_version = 2` и обратной совместимостью со старым форматом без `schema_version`, а также `migrate_in_repo_knowledge` для переноса `.agents/knowledge` в центральное хранилище. |
| `crates/ci-core/src/git.rs` | Создан | Реализован модуль `git` (D2, D3): трейт `GitBackend`, `SystemGitBackend` (системный `git` с `-z` / `--porcelain=v1 -z` и нормализацией путей к `/`), `parse_porcelain_z`, `parse_nul_paths` и `check_card_staleness` (сверка `commit_sha`/`blob_sha`, корректная обработка пустого `recorded_blob_sha`, флаг `stale` и `diff --stat`). |
| `crates/ci-core/src/telemetry.rs` | Создан | Реализован модуль `telemetry` (D28–D30, O6): трейт `TelemetrySink`, `JsonlTelemetrySink` (запись в `<data_dir>/context-init/telemetry/YYYY-MM.jsonl` со `schema_version = 1` только при `enabled = true`), сигналы `OperationalSignal`, `EfficiencySignal`, `LlmSignal`, `SessionPatternSignal`, `FeedbackSignal { tool, verdict, note }` с анонимизированным `project-id` и генератор дайджеста `render_telemetry_report(events, include_notes)` со скрытием `note` по умолчанию. |
| `crates/ci-core/src/lib.rs` | Изменен | Экспортированы модули `config`, `storage`, `openviking`, `git`, `telemetry`, публичные типы/функции и добавлены 11 комплексных unit-тестов для всех подсистем и граничных случаев. |

---

## 2. Статус критериев приемки (Acceptance Criteria)

- [x] `rtk cargo check -p ci-core` и `rtk cargo check --workspace` завершаются с кодом 0.
- [x] `rtk cargo clippy -p ci-core --all-targets -- -D warnings` и по всему `--workspace` не выдают предупреждений (`cargo clippy: No issues found`).
- [x] `rtk cargo nextest run -p ci-core` проходит успешно (`15 passed`, `53 passed` по всему `--workspace`).
- [x] Отсутствуют упоминания запрещенных доменных терминов (`rtk rg -i '1[cс]' Cargo.toml crates/ src/` → exit code 1).
- [x] Итоговый результат и diff зафиксированы в `/home/gidragir/projects/context-init/reports/subagents/ci-core_summary.md`.
