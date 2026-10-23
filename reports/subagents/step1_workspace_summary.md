# Шаг 1 вехи v0.3 «Фундамент»: Каркас Cargo Workspace и профили сборки

## 1. Сводка изменений

| Файл | Действие | Описание |
| :--- | :--- | :--- |
| `Cargo.toml` | Изменен | Сконфигурирован `[workspace]` (`resolver = "2"`, `members = ["crates/ci-core", "crates/ci-llm", "crates/ci-mcp", "."]`), `[workspace.package]` (`version = "0.3.0"`, `edition = "2021"`, `authors = ["gidragir"]`), `[workspace.dependencies]` (включая `toml`, `dirs-next`, `sha2` и внутренние крейты), подключены `ci-core`, `ci-llm`, `ci-mcp` в корневой пакет `context-init`, добавлены профили `dev`, `dev.package."*"`, `test`, `release`, `dist` (D24, D25). |
| `crates/ci-core/Cargo.toml` | Создан | Манифест библиотечного крейта `ci-core` с workspace-наследованием метаданных и базовых зависимостей (`serde`, `serde_json`, `anyhow`, `toml`, `dirs-next`, `sha2`, `schemars`). |
| `crates/ci-core/src/lib.rs` | Создан | Базовый модуль `ci-core` с doc-комментариями, константой `VERSION`, функцией `crate_info()` и smoke unit-тестом. |
| `crates/ci-llm/Cargo.toml` | Создан | Манифест библиотечного крейта `ci-llm` с зависимостью `ci-core = { path = "../ci-core" }` и общими workspace-зависимостями (`anyhow`, `serde`, `serde_json`, `ureq`). |
| `crates/ci-llm/src/lib.rs` | Создан | Базовый модуль `ci-llm` с doc-комментариями, константой `VERSION`, функцией `crate_info()` (связанной с `ci_core::VERSION`) и smoke unit-тестом. |
| `crates/ci-mcp/Cargo.toml` | Создан | Манифест библиотечного крейта `ci-mcp` с зависимостями `ci-core = { path = "../ci-core" }`, `ci-llm = { path = "../ci-llm" }` и общими workspace-зависимостями (`rmcp`, `tokio`, `schemars`, `tiktoken-rs`, `serde`, `serde_json`, `anyhow`). |
| `crates/ci-mcp/src/lib.rs` | Создан | Базовый модуль `ci-mcp` с doc-комментариями, константой `VERSION`, функцией `crate_info()` (связанной с `ci_core` и `ci_llm`) и smoke unit-тестом. |
| `src/main.rs` | Изменен | Заменена захардкоженная версия `version = "0.2.0"` в `#[command(...)]` на `version` без литерала (D27); добавлен unit-тест `test_cli_version_matches_cargo_pkg_version_and_workspace_crates`. |
| `src/detector.rs` | Изменен | В `parse_cargo_toml` добавлен полноценный разбор через `toml::Value` (с корректной обработкой `version.workspace = true`, `[workspace.package]`, `[workspace.dependencies]` и секции `[package]`). В `discover_subsystems` добавлены: пропуск корневого элемента `"."` в `workspace_members`, дедупликация плоских `src/main.rs` и `src/lib.rs` в единый модуль `src/core`, извлечение `package.name` из `Cargo.toml` подкрейтов для `test_cmd`; добавлены unit-тесты `test_workspace_root_dot_member_does_not_create_empty_subsystem`, `test_flat_src_merges_main_and_lib_into_single_core_module`, `test_parse_cargo_toml_workspace_inheritance_and_sections`. |
| `src/generator.rs` | Изменен | Устранено предупреждение `clippy::cloned_ref_to_slice_refs` в тестовом модуле (`std::slice::from_ref(&sub)`). |
| `tests/regression_benchmark.rs` | Изменен | Устранено предупреждение `clippy::single_char_add_str` (`md.push('\n')`). |

---

## 2. Статус критериев приемки (Acceptance Criteria)

- [x] `rtk cargo check --workspace` завершается с кодом 0.
- [x] `rtk cargo check -p ci-core -p ci-llm -p ci-mcp -p context-init` завершается с кодом 0.
- [x] `rtk cargo check --profile release --workspace` и `rtk cargo check --profile dist --workspace` завершаются с кодом 0.
- [x] `rtk cargo clippy --workspace --all-targets -- -D warnings` не выдает предупреждений (`cargo clippy: No issues found`).
- [x] `rtk cargo nextest run --workspace` проходит успешно (`39 passed (6 binaries, 0.209s)`), `rtk cargo test --workspace` также проходит (`39 passed (10 suites, 0.24s)`).
- [x] Итоговый результат зафиксирован в `/home/gidragir/projects/context-init/reports/subagents/step1_workspace_summary.md`.

---

## 3. Вывод `rtk git status`

```text
* main...origin/main [ahead 7]
 M .agents/knowledge/L0_index.json
 M .agents/knowledge/L0_index.md
 M .agents/skills/context-init/SKILL.md
 M AGENTS.md
 M Cargo.lock
 M Cargo.toml
 M reports/BENCHMARK_REPORT.md
 M reports/benchmark_latest.json
 M src/detector.rs
 M src/generator.rs
 M src/main.rs
 M src/mcp.rs
 M src/service.rs
 M src/types.rs
 M tests/regression_benchmark.rs
?? crates/
?? milestone_decisions.md
?? references/
?? reports/subagents/
```
