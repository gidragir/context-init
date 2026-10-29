# План завершения дизайна `context-init` (v0.3–v0.6)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Довести реализацию `context-init` до 100% соответствия спецификациям [milestone_decisions.md](../../milestone_decisions.md) (`D1`–`D31`, `O1`–`O7`), [references/openviking_spec.md](../../references/openviking_spec.md) (§1–§3) и архитектурному манифесту оркестрации по четырём последовательным этапам (`v0.3` финализация → `v0.4` OKF + языки/метаданные + scan → `v0.5` задачи и workspace → `v0.6` поиск BM25).

**Architecture:** Монорепозиторий Cargo Workspace (`crates/ci-core`, `crates/ci-llm`, `crates/ci-mcp`, бинарный крейт `context-init`) с единым конвейером между CLI и глобальным MCP-сервером. Каждый этап завершается независимым гейтом верификации (`cargo nextest`, `clippy -D warnings`, `regression_benchmark`) и коммитом актуализированной базы знаний в `.context-init/knowledge/`.

**Tech Stack:** Rust 2021, `clap`, `rmcp`, `serde`/`serde_json`, `toml`/`toml_edit`, `syn`, `tiktoken-rs`, `sha2`, `ignore`/`globset` (Этап 2), `quick-xml` (Этап 2), `tantivy`/`rust-stemmers` (Этап 4), `mise`, `cargo-nextest`, `cargo-dist`, `release-plz`.

**Spec:**
- `milestone_decisions.md`
- `references/openviking_spec.md`
- `reports/subagents/context-init_verification.md`

## Global Constraints

- Перед вызовом `cargo` экспортировать `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH"`.
- Все терминальные команды выполнять строго с префиксом `rtk` (`rtk cargo ...`, `rtk git ...`, `rtk rg ...`, `rtk fd ...`). Использование GNU `find` и GNU `grep` категорически запрещено.
- Инвариант нейтральности: 0 упоминаний «1С» / «1C» в коде, комментариях, тестах, документации и шаблонах (`rtk rg -i '1[cс]' crates src tests`).
- Все тесты, создающие базы знаний или конфиги, обязаны изолировать `XDG_DATA_HOME`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME` и `HOME` во временных директориях.
- Кроссплатформенность: пути внутри базы знаний и карточек нормализуются к `/`, парсеры устойчивы к CRLF, отсутствуют Unix-only вызовы без `#[cfg(unix)]`.
- Жёсткий потолок токенов ответа MCP (`max_response_tokens`, по умолчанию `4000`, минимум `64`) с курсорной пагинацией (`next_cursor`) на всех MCP-инструментах.
- Коммиты в формате Conventional Commits через локальный `rtk oco` (или конвенциональные сообщения при работе в изолированной ветке); после структурных изменений выполнять `rtk cargo run -- update .` и коммитить `.context-init/knowledge/` в том же PR.

## Review Focus

1. **Отсутствующий `L0_index.json` при вызове `context_check` без аргумента `path` (`M1`):** Вместо маскировки ошибки загрузки под `DRIFT: up_to_date=0, missing=[], untracked=[]` и CLI, и MCP должны использовать общий `pipeline::run_check`, возвращающий явную ошибку отсутствия базы (с `storage_mismatch_hint`, если база есть во втором режиме) и учитывающий `git_stale`.
2. **Опечатка в `--model <alias>` (`M7`) и невалидный второй харнесс при `setup all --write` (`M4`):** Неизвестный алиас модели в CLI/MCP-переопределении обязан немедленно завершать команду ошибкой валидации, а `setup all --write` обязан построить `SetupPlan` для всех целевых харнессов в памяти до первой записи на диск.
3. **Стабильность repo-storage при генерации OKF frontmatter (`D1a` + `D2`):** Включение YAML frontmatter (`OkfCard`) в `subsystems/*.md` не должно нарушать идемпотентность `update` на неизменённом дереве (`update_twice_is_noop`): `commit_sha` и `updated_at` обновляются только при реальном изменении `blob_sha` или тела карточки.
4. **Недоверенные пути и огромные репозитории в `scan` и `mdo` (`D8`, `D9`):** `scan_project` и `discover_mdo_subsystems` обязаны учитывать `.gitignore` и отвергать симлинки/пути, выходящие за пределы `root`.
5. **Соблюдение `max_response_tokens` на новых инструментах (`D5`, `D6`, `D9`, `D18`, `D19`):** Дайджест задачи (`context_task`), карта мультирепозитория (`context_workspace`), отчёт сканирования (`context_scan`) и выдача поиска (`context_search`) никогда не возвращают ответ длиннее `max_response_tokens` (проверяется тестом при бюджете `256` токенов).

---

## Этап 1: Финализация вехи v0.3 «Фундамент»

### Task 1: Единый конвейер `check` (CLI ↔ MCP), строгая валидация `--model` и `rtk`-префикс в `AGENTS.md` (`M1`, `M7`, `D31`)

**Files:**
- Modify: `crates/ci-mcp/src/pipeline.rs`
- Modify: `crates/ci-mcp/src/handlers.rs:173-235`
- Modify: `src/cli/check.rs`
- Modify: `crates/ci-core/src/analysis/generator.rs:551-609`
- Test: `crates/ci-mcp/src/lib.rs`, `tests/cli.rs`

**Interfaces:**
- Produces in `ci_mcp::pipeline`:
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
  pub struct FullDriftReport {
      pub has_drift: bool,
      pub missing_in_fs: Vec<String>,
      pub untracked_in_knowledge: Vec<String>,
      pub up_to_date_count: usize,
      pub git_stale: bool,
      pub stale_subsystems: Vec<String>,
  }

  pub fn run_check(project: &ResolvedProject) -> Result<FullDriftReport>;
  pub fn validate_model_override(config: &ci_core::config::Config, model_override: Option<&str>) -> Result<()>;
  ```

- [ ] **Step 1: Write the failing tests**
  In `crates/ci-mcp/src/lib.rs`, add test `check_drift_errors_on_missing_index_with_mismatch_hint`:
  - Call `ContextHandler::check_drift(&project_without_kb, None)` → `Err` whose message contains `"Knowledge base not found"` (and `"migrate --to"` when the other storage mode has an index).
  In `tests/cli.rs`, add tests:
  - `cli_init_rejects_unknown_model_alias`: run `context-init init . --model nosuch` → non-zero exit code, stderr contains `"Unknown model alias 'nosuch'"`.
  - `cli_agents_md_contains_rtk_fallback_commands`: after `init .`, `AGENTS.md` contains `"rtk context-init pack <subsystem_id> --compress"` and `"rtk context-init check --compact"`.

- [ ] **Step 2: Run tests to verify they fail**
  Run: `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && rtk cargo nextest run -p ci-mcp -p context-init`
  Expected: FAIL (`check_drift` returns `Ok("DRIFT: up_to_date=0...")`, `init --model nosuch` exits 0, `AGENTS.md` lacks `rtk ` prefix).

- [ ] **Step 3: Implement `run_check`, `validate_model_override`, and `rtk` fallback wording**
  - In `crates/ci-mcp/src/pipeline.rs`:
    - Implement `validate_model_override`: split `model_override` by `,`, trim each alias, verify `alias.eq_ignore_ascii_case(ROLE_NONE) || config.models.contains_key(alias)`, else `bail!("Unknown model alias '{}' in --model override", alias)`. Call it at the start of `analyze()` and in `GlobalArgs::load_project` (or after `apply_cli_overrides` via `config.validate()`).
    - Implement `run_check(project: &ResolvedProject) -> Result<FullDriftReport>`: load `L0Index` (attaching `storage_mismatch_hint(project)` to the error if missing), run `detector::analyze_project_with`, call `generator::audit`, collect `stale_subsystems` via `project.check_subsystem_staleness(s).0`, compute `has_drift = !report.missing_in_fs.is_empty() || !report.untracked_in_knowledge.is_empty() || !stale_subsystems.is_empty()`.
  - Wire `pipeline::run_check` into both `ContextHandler::check_drift` (`crates/ci-mcp/src/handlers.rs`) and `src/cli/check.rs`.
  - In `crates/ci-core/src/analysis/generator.rs:575`: update the CLI fallback line in `update_or_create_root_agents_md` to ``Без доступа к MCP (субагенты, скрипты): `rtk context-init pack <subsystem_id> --compress` и `rtk context-init check --compact`.``

- [ ] **Step 4: Run tests to verify they pass**
  Run: `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && rtk cargo nextest run --workspace`
  Expected: PASS.

- [ ] **Step 5: Commit**
  Run: `rtk git add crates/ci-core/src/analysis/generator.rs crates/ci-mcp/src/pipeline.rs crates/ci-mcp/src/handlers.rs crates/ci-mcp/src/lib.rs src/cli/check.rs tests/cli.rs && rtk git commit -m "fix(check,cli): unify CLI/MCP drift check, reject unknown --model alias, add rtk fallback in AGENTS.md"`

---

### Task 2: Безопасная и атомарная запись в `setup` и `telemetry` (`M4`, `M6`, `M8`, `M9`)

**Files:**
- Modify: `src/cli/setup/mod.rs:101-149`
- Modify: `src/cli/setup/merge.rs:44-68, 145-168`
- Modify: `src/cli/telemetry.rs:134-160`
- Test: `src/cli/setup/merge.rs`, `tests/cli.rs`

**Interfaces:**
- Produces in `src/cli/setup/merge.rs`:
  ```rust
  pub fn atomic_write_str(path: &Path, content: &str) -> Result<()>;
  ```
- Produces in `src/cli/telemetry.rs`:
  ```rust
  pub fn create_secure_temp_markdown(content: &str) -> Result<std::path::PathBuf>;
  ```

- [ ] **Step 1: Write the failing tests**
  In `src/cli/setup/merge.rs`, add tests:
  - `merge_toml_converts_inline_tables`: input `mcp_servers = { other = { command = "x" } }\n` and `[mcp_servers]\ncontext-init = { command = "/old" }\n` both succeed and preserve `other`.
  - `atomic_write_str_replaces_file_preserving_permissions`: writes via temp file + rename and keeps `0o600` mode on Unix.
  In `tests/cli.rs`, add test:
  - `cli_setup_all_write_aborts_before_any_write_if_one_harness_config_is_invalid`: create valid `~/.gemini/antigravity/mcp_config.json` and malformed `~/.cursor/mcp.json` (`{bad`); run `setup all --write` → exits 1 and `~/.gemini/antigravity/mcp_config.json` is untouched.
  In `src/cli/telemetry.rs`, add test:
  - `create_secure_temp_markdown_uses_create_new_and_unique_name`: two consecutive calls create distinct files with `0o600` permissions (on Unix) and write the digest.

- [ ] **Step 2: Run tests to verify they fail**
  Run: `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && rtk cargo nextest run --workspace`
  Expected: FAIL (`merge_toml` fails on inline table, `setup all --write` writes antigravity before failing on cursor).

- [ ] **Step 3: Implement pre-planning, inline table conversion, atomic write, and secure temp file**
  - In `src/cli/setup/merge.rs`:
    - In `merge_toml`: if `doc["mcp_servers"]` or `servers[SERVER_NAME]` is an `Item::Value(Value::InlineTable(it))`, convert it in place to `Item::Table(it.into_table())` before calling `.as_table_mut()`.
    - Implement `atomic_write_str(path: &Path, content: &str) -> Result<()>`: create sibling `<name>.tmp.<pid>.<nanos>` using `OpenOptions::new().write(true).create_new(true)`, copy permissions from `path` if `path.exists()`, write `content`, flush, and `fs::rename(&tmp, path)`. Use `atomic_write_str` in `PlannedWrite::apply`.
  - In `src/cli/setup/mod.rs`: collect `let plans: Vec<SetupPlan> = harnesses.into_iter().map(|h| plan_setup(h, &dirs, &exe)).collect::<Result<_>>()?;` before the output/write loop.
  - In `src/cli/telemetry.rs`: implement `create_secure_temp_markdown` using `OpenOptions::new().write(true).create_new(true)` (with `mode(0o600)` on `#[cfg(unix)]`) and unique `pid + nanos` suffix.

- [ ] **Step 4: Run tests to verify they pass**
  Run: `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && rtk cargo nextest run --workspace`
  Expected: PASS.

- [ ] **Step 5: Commit**
  Run: `rtk git add src/cli/setup/mod.rs src/cli/setup/merge.rs src/cli/telemetry.rs tests/cli.rs && rtk git commit -m "fix(setup,telemetry): atomic config writes, pre-plan setup all, inline TOML tables, secure temp file"`

---

### Task 3: Таск-раннер `mise.toml`, очистка `lefthook.yml`, метаданные публикации `crates.io` и `binstall` (`D26`, `D27`, `O2`)

**Files:**
- Create: `mise.toml`
- Modify: `lefthook.yml`
- Modify: `Cargo.toml`
- Modify: `crates/ci-core/Cargo.toml`, `crates/ci-llm/Cargo.toml`, `crates/ci-mcp/Cargo.toml`
- Test: `tests/cli.rs`

**Interfaces:**
- `mise.toml` defines tasks: `build`, `test`, `lint`, `fmt`, `check`, `dist`, `audit`.
- `[workspace.package]` in `Cargo.toml` defines `version = "0.3.0"`, `edition = "2021"`, `authors = ["gidragir"]`, `license = "MIT"`, `repository = "https://github.com/gidragir/context-init"`, `homepage = "https://github.com/gidragir/context-init"`.
- Workspace internal crates set `package = "context-init-core"` / `"context-init-llm"` / `"context-init-mcp"` or explicit `description`/`license.workspace = true`/`repository.workspace = true` so `cargo package --workspace --allow-dirty` / `release-plz` has complete metadata (`O2`).
- `[package.metadata.binstall]` in root `Cargo.toml` configures `pkg-url` and `pkg-fmt = "tgz"` (`D27`).

- [ ] **Step 1: Write the failing test**
  In `tests/cli.rs`, add test `workspace_task_runner_and_packaging_metadata_invariants`:
  - Assert `mise.toml` exists at `CARGO_MANIFEST_DIR` and parses as valid TOML containing `[tasks.build]`, `[tasks.test]`, `[tasks.lint]`, `[tasks.dist]`.
  - Assert `lefthook.yml` does not contain `"sync-binary"`.
  - Assert root `Cargo.toml` contains `[package.metadata.binstall]` and `license = "MIT"`.

- [ ] **Step 2: Run test to verify it fails**
  Run: `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && rtk cargo nextest run -E 'test(workspace_task_runner_and_packaging_metadata_invariants)'`
  Expected: FAIL (`mise.toml` does not exist).

- [ ] **Step 3: Create `mise.toml`, update `lefthook.yml` and `Cargo.toml` manifests**
  - Create `mise.toml` with tasks `build`, `test`, `lint`, `fmt`, `check`, `dist`, `audit`.
  - Remove `sync-binary` from `lefthook.yml`.
  - Add `license`, `repository`, `homepage` to `[workspace.package]` in `Cargo.toml`, inherit them in `crates/ci-*/Cargo.toml` and root `Cargo.toml`, and add `[package.metadata.binstall]` to root `Cargo.toml`.

- [ ] **Step 4: Run test to verify it passes**
  Run: `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && rtk cargo nextest run --workspace`
  Expected: PASS.

- [ ] **Step 5: Commit**
  Run: `rtk git add mise.toml lefthook.yml Cargo.toml crates/ci-core/Cargo.toml crates/ci-llm/Cargo.toml crates/ci-mcp/Cargo.toml tests/cli.rs && rtk git commit -m "chore(build): add mise.toml tasks, binstall/crates.io metadata, drop lefthook sync-binary (D26, D27)"`

---

### Task 4: GitHub Actions CI и пайплайн релизов (`D26`, `D27`)

**Files:**
- Create: `.github/workflows/ci.yml`
- Create: `.github/workflows/release-plz.yml`
- Create: `.github/workflows/release.yml`
- Create: `release-plz.toml`
- Create: `dist-workspace.toml`
- Test: `tests/cli.rs`

**Interfaces:**
- `.github/workflows/ci.yml`: triggers on `push` (`main`) and `pull_request`; runs `fmt`, `clippy --workspace --all-targets -- -D warnings`, `nextest` on `ubuntu-latest`, `macos-latest`, `windows-latest` with `Swatinem/rust-cache@v2`, plus `regression_benchmark` and neutrality check.
- `dist-workspace.toml`: configures `cargo-dist` for `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`, installers `["shell", "powershell", "homebrew"]`, `tap = "gidragir/homebrew-tap"`.
- `release-plz.toml` + `.github/workflows/release-plz.yml` + `.github/workflows/release.yml` (including Scoop bucket update job for `gidragir/scoop-bucket`).

- [ ] **Step 1: Write the failing test**
  In `tests/cli.rs`, add test `ci_and_release_configuration_files_exist_and_cover_d26_d27`:
  - Verify `.github/workflows/ci.yml` contains `ubuntu-latest`, `macos-latest`, `windows-latest`, `Swatinem/rust-cache`, `clippy`, `nextest`, `regression_benchmark`.
  - Verify `dist-workspace.toml` parses as valid TOML and lists all 5 targets from D27 and `gidragir/homebrew-tap`.
  - Verify `release-plz.toml` parses as valid TOML.

- [ ] **Step 2: Run test to verify it fails**
  Run: `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && rtk cargo nextest run -E 'test(ci_and_release_configuration_files_exist_and_cover_d26_d27)'`
  Expected: FAIL.

- [ ] **Step 3: Create `.github/workflows/{ci,release-plz,release}.yml`, `dist-workspace.toml`, and `release-plz.toml`**

- [ ] **Step 4: Run test to verify it passes**
  Run: `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && rtk cargo nextest run --workspace`
  Expected: PASS.

- [ ] **Step 5: Commit**
  Run: `rtk git add .github/ dist-workspace.toml release-plz.toml tests/cli.rs && rtk git commit -m "ci(release): add multi-OS CI workflow, cargo-dist config, and release-plz pipeline (D26, D27)"`

---

### Task 5: Гейт `context-audit` для вехи v0.3 (`milestone_decisions.md` §3)

**Files:**
- Modify: `tests/regression_benchmark.rs`
- Modify: `reports/BENCHMARK_REPORT.md`, `reports/benchmark_latest.json`
- Modify: `.context-init/knowledge/**`, `AGENTS.md`

**Interfaces:**
- Adds `V03Metrics` struct to `BenchmarkReport` in `tests/regression_benchmark.rs`:
  ```rust
  struct V03Metrics {
      mcp_cold_start_ms: f64,
      router_fallback_avg_ms: f64,
      budget_compliance_percent: f64,
  }
  ```

- [ ] **Step 1: Write the failing assertions in `tests/regression_benchmark.rs`**
  - Measure cold start of `ContextInitServer::new` + `resolve_context` (`assert!(mcp_cold_start_ms < 50.0)`).
  - Measure `RoleRouter::execute` across a 2-step fallback chain (`failing -> fallback` and `failing -> none`) over 10 iterations (`assert!(router_fallback_avg_ms < 5.0)`).
  - Set `project.config.max_response_tokens = 256` and invoke `get_subsystems`, `get_subsystem`, `pack`, `check_drift`, `scan`, `task`, `search` on a multi-subsystem workspace; assert `count_tokens(&reply) <= 256` for 100% of calls (`assert_eq!(budget_compliance_percent, 100.0)`).

- [ ] **Step 2: Run benchmark test to verify compilation and execution**
  Run: `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && CONTEXT_INIT_WRITE_BENCHMARK=1 rtk cargo test --test regression_benchmark -- --nocapture`
  Expected: PASS and writes updated `reports/BENCHMARK_REPORT.md` and `reports/benchmark_latest.json`.

- [ ] **Step 3: Refresh repo knowledge base and verify zero drift**
  Run: `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && rtk cargo run -- update . && rtk cargo run -- check . --exit-code --compact`
  Expected: `OK: ... subsystems up-to-date`.

- [ ] **Step 4: Run full workspace gate**
  Run: `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && rtk cargo clippy --workspace --all-targets -- -D warnings && rtk cargo nextest run --workspace`
  Expected: PASS with 0 warnings.

- [ ] **Step 5: Commit**
  Run: `rtk git add tests/regression_benchmark.rs reports/ .context-init/ AGENTS.md && rtk git commit -m "test(audit): add v0.3 context-audit metrics (MCP cold start, router fallback, token budget compliance)"`

---

## Этап 2: Веха v0.4 «Metadata-кодовые базы, OKF Sidecar, двухфазный Scan/Init и языковые адаптеры»

### Task 6: Генерация OKF Sidecar Frontmatter (`OkfCard`) в L1-карточках и ленивая Git Stale-валидация + `auto_refresh` (`D2`, `openviking_spec.md` §2–3)

**Files:**
- Modify: `crates/ci-core/src/config.rs` (`Config.auto_refresh: bool`, env `CONTEXT_INIT_AUTO_REFRESH`)
- Modify: `crates/ci-core/src/analysis/generator.rs` (`render_subsystem_card`, `generate`, `update`)
- Modify: `crates/ci-mcp/src/handlers.rs` (`get_subsystem`, `persist_submission`)
- Test: `crates/ci-core/src/analysis/generator.rs`, `crates/ci-mcp/src/lib.rs`

**Interfaces:**
- Produces in `ci_core::analysis::generator`:
  ```rust
  pub fn render_subsystem_card_okf(
      sub: &SubsystemInfo,
      analysis: &ProjectAnalysis,
      source_shas: Option<(String, String)>, // (blob_sha, commit_sha)
      role: &str,
      model: &str,
  ) -> String;
  ```
- `Config.auto_refresh: bool` (default `false`).

- [ ] **Step 1: Write the failing tests**
  - `generated_l1_cards_parse_as_valid_okf_cards`: after `generator::generate`, every `subsystems/*.md` parses via `OkfCard::parse(&text)`, has `frontmatter.directory` matching the subsystem path, non-empty `source.blob_sha`, and `card.l0_abstract()` returns the subsystem summary.
  - `update_twice_with_okf_cards_ remains_noop`: running `update` twice on an unchanged git repo leaves all `subsystems/*.md` byte-identical (preserving `commit_sha` when `blob_sha` and body are unchanged).
  - `mcp_get_subsystem_reports_stale_diff_stat_and_auto_refreshes`: in a temp git repo, `init`, modify a source file in a subsystem and commit; `get_subsystem` with `auto_refresh = false` returns `stale=true` in footer and `diff --stat` block; with `auto_refresh = true`, refreshes the card's `blob_sha`/`commit_sha` and clears `stale`.

- [ ] **Step 2: Run tests to verify they fail**
  Run: `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && rtk cargo nextest run -p ci-core -p ci-mcp`
  Expected: FAIL (`OkfCard::parse` fails on generated L1 cards because they lack `---` frontmatter).

- [ ] **Step 3: Implement OKF frontmatter generation, staleness reporting, and `auto_refresh`**
  - In `generator.rs`: wrap `render_subsystem_card` body in `OkfCard` frontmatter using `SystemGitBackend` (falling back to a deterministic SHA256 of the file list when outside a git repo). In `update`, if an existing card's `blob_sha` and markdown body match the newly rendered ones, retain its `commit_sha` so `write_if_changed` writes nothing.
  - In `crates/ci-mcp/src/handlers.rs`: in `get_subsystem`, when `check_subsystem_staleness` returns `(true, Some(stat))`, if `project.config.auto_refresh` is enabled, re-run single-subsystem update/enrichment and persist the refreshed card; otherwise append `\n## Git Staleness (diff --stat)\n\`\`\`text\n{stat}\n\`\`\`\n` and pass `stale = true` to `paginate_content`.

- [ ] **Step 4: Run tests to verify they pass**
  Run: `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && rtk cargo nextest run --workspace`
  Expected: PASS.

- [ ] **Step 5: Commit**
  Run: `rtk git add crates/ci-core/ crates/ci-mcp/ && rtk git commit -m "feat(kb): emit OpenViking OKF frontmatter in L1 cards and support lazy git stale auto_refresh (D2)"`

---

### Task 7: Двухфазный `scan` / `init` и управление Scope на базе `ignore` (`D9`)

**Files:**
- Modify: `Cargo.toml`, `crates/ci-core/Cargo.toml` (add `ignore = "0.4"`, `globset = "0.4"`)
- Create: `crates/ci-core/src/analysis/scan.rs`
- Modify: `crates/ci-core/src/config.rs` (`ScopeConfig`)
- Modify: `crates/ci-core/src/analysis/detector.rs`
- Modify: `crates/ci-mcp/src/handlers.rs` (`ContextHandler::scan`)
- Create: `src/cli/scan.rs`
- Modify: `src/cli/init_update.rs`, `src/cli/mod.rs`
- Test: `crates/ci-core/src/analysis/scan.rs`, `tests/cli.rs`

**Interfaces:**
- Produces in `ci_core::config`:
  ```rust
  #[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
  #[serde(default)]
  pub struct ScopeConfig {
      pub include: Vec<String>,
      pub exclude: Vec<String>,
      pub extensions: Vec<String>,
  }
  ```
- Produces in `ci_core::analysis::scan`:
  ```rust
  pub fn scan_project(root: &Path, scope: &ScopeConfig) -> Result<ScanReport>;
  pub fn render_scan_markdown(project_id: &str, root: &Path, report: &ScanReport) -> String;
  ```
- CLI: `context-init scan [PATH] [--json]` and `context-init init [PATH] [--include <GLOB>...] [--exclude <GLOB>...] [--ext <EXT>...]`.

- [ ] **Step 1: Write the failing tests**
  - `scan_respects_gitignore_and_computes_token_shares`: create temp repo with `.gitignore` ignoring `build/` and `vendor/`; run `scan_project` → `build/` and `vendor/` are excluded, directory and extension stats include files, lines, tokens, and `share_percent` summing to ~100%, plus `suggested_scope`.
  - `cli_init_with_scope_flags_persists_scope_and_filters_subsystems`: run `context-init init . --include "crates/ci-core/**" --exclude "**/tests/**" --ext rs` → writes `[scope]` into `.context-init.toml` and detects only `crates/ci-core` Rust files.

- [ ] **Step 2: Run tests to verify they fail**
  Run: `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && rtk cargo nextest run --workspace`
  Expected: FAIL.

- [ ] **Step 3: Implement `ScopeConfig`, `scan_project` with `ignore::WalkBuilder`, and CLI/MCP integration**
  - Use `ignore::WalkBuilder` in both `scan_project` and `detector.rs` so `.gitignore` and `ScopeConfig` (`include`, `exclude`, `extensions` via `globset::GlobSet`) are respected everywhere (`init`, `update`, `pack`, `scan`).
  - Add `context-init scan` subcommand and `--include`/`--exclude`/`--ext` flags on `init`.

- [ ] **Step 4: Run tests to verify they pass**
  Run: `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && rtk cargo nextest run --workspace`
  Expected: PASS.

- [ ] **Step 5: Commit**
  Run: `rtk git add Cargo.toml Cargo.lock crates/ src/ tests/ && rtk git commit -m "feat(scan): two-phase scan/init with .gitignore walker and persistent [scope] filters (D9)"`

---

### Task 8: Трейт `LanguageAdapter` и адаптеры `rust`, `python`, `bsl`, `.os` (`D7`)

**Files:**
- Create: `crates/ci-core/src/analysis/lang/mod.rs`
- Create: `crates/ci-core/src/analysis/lang/rust.rs`
- Create: `crates/ci-core/src/analysis/lang/python.rs`
- Create: `crates/ci-core/src/analysis/lang/bsl.rs`
- Modify: `crates/ci-core/src/analysis/mod.rs`, `crates/ci-core/src/analysis/pack.rs`, `crates/ci-core/src/analysis/detector.rs`
- Test: `crates/ci-core/src/analysis/lang/mod.rs`

**Interfaces:**
- Produces in `ci_core::analysis::lang`:
  ```rust
  pub trait LanguageAdapter: Send + Sync {
      fn language_id(&self) -> &'static str;
      fn extensions(&self) -> &'static [&'static str];
      fn extract_symbols(&self, source: &str) -> Vec<String>;
      fn compress_source(&self, source: &str) -> String;
  }
  pub fn adapter_for_path(path: &Path) -> Option<&'static dyn LanguageAdapter>;
  ```

- [ ] **Step 1: Write the failing tests**
  - `bsl_adapter_extracts_exported_procedures_and_compresses_bodies`: feed a `.bsl` / `.os` module containing `&НаСервере`, `Процедура ОбработатьДанные(Парам1) Экспорт ... КонецПроцедуры`, `Функция Внутренняя() ... КонецФункции`, and English `Procedure RunTask(Val X) Export ... EndProcedure` → `extract_symbols` returns the exported signatures, and `compress_source` keeps directives, comments, and signatures while replacing bodies with `⋮----` and saving `>= 60%` tokens.
  - `rust_and_python_adapters_preserve_existing_pack_compression`: verify `adapter_for_path` dispatches `.rs`, `.py`, `.bsl`, `.os` and `pack_subsystem` uses `adapter_for_path`.

- [ ] **Step 2: Run tests to verify they fail**
  Run: `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && rtk cargo nextest run -p ci-core`
  Expected: FAIL.

- [ ] **Step 3: Implement `LanguageAdapter` trait and `RustAdapter`, `PythonAdapter`, `BslAdapter`**
  - Move Rust (`syn`) and Python compression/symbol extraction into `lang/{rust,python}.rs`.
  - Implement `BslAdapter` in `lang/bsl.rs` supporting Cyrillic and English keywords (`Процедура`/`КонецПроцедуры`/`Функция`/`КонецФункции`, `Procedure`/`EndProcedure`/`Function`/`EndFunction`, `Экспорт`/`Export`, `&...` directives) and CRLF line endings.

- [ ] **Step 4: Run tests to verify they pass**
  Run: `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && rtk cargo nextest run --workspace`
  Expected: PASS.

- [ ] **Step 5: Commit**
  Run: `rtk git add crates/ci-core/ && rtk git commit -m "feat(lang): introduce LanguageAdapter trait and implement rust, python, and bsl/os adapters (D7)"`

---

### Task 9: Адаптер `mdo` (XML), подсистемы конфигурации и ленивые L2-карточки объектов метаданных (`D7`, `D8`)

**Files:**
- Modify: `Cargo.toml`, `crates/ci-core/Cargo.toml` (add `quick-xml = "0.37"`)
- Create: `crates/ci-core/src/analysis/lang/mdo.rs`
- Modify: `crates/ci-core/src/analysis/detector.rs`, `crates/ci-core/src/analysis/generator.rs`, `crates/ci-mcp/src/handlers.rs`
- Test: `crates/ci-core/src/analysis/lang/mdo.rs`, `crates/ci-mcp/src/lib.rs`

**Interfaces:**
- Produces in `ci_core::analysis::lang::mdo`:
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
  pub struct MdoObject {
      pub kind: String,
      pub name: String,
      pub synonym: Option<String>,
      pub attributes: Vec<String>,
      pub tabular_sections: Vec<(String, Vec<String>)>,
      pub content_refs: Vec<String>,
      pub child_subsystems: Vec<String>,
      pub exported_methods: Vec<String>,
  }
  pub fn parse_mdo_xml(xml: &str) -> Result<MdoObject>;
  pub fn render_mdo_l2_card(obj: &MdoObject) -> String;
  ```

- [ ] **Step 1: Write the failing tests**
  - `parse_mdo_xml_extracts_subsystem_and_metadata_object_fields`: test parsing a configuration subsystem `.mdo` (`<mdclass:Subsystem>`, `<content>`, `<subsystems>`) and a catalog/document `.mdo` (`<attributes>`, `<tabularSections>`, synonym).
  - `detector_maps_configuration_subsystems_and_lazy_l2_object_cards`: create synthetic metadata tree (`src/Subsystems/Billing/Billing.mdo`, `src/Catalogs/Items/Items.mdo`, `src/Catalogs/Items/ObjectModule.bsl`, `src/CommonModules/ Common/Module.bsl`); run `analyze_project` → L1 subsystems correspond to `Billing` and standalone metadata groups; requesting `context_get_subsystem` for `catalog_items` lazily generates and returns the L2 card with attributes, tabular sections, and exported `.bsl` methods.

- [ ] **Step 2: Run tests to verify they fail**
  Run: `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && rtk cargo nextest run -p ci-core -p ci-mcp`
  Expected: FAIL.

- [ ] **Step 3: Implement `mdo.rs`, metadata subsystem discovery in `detector.rs`, and lazy L2 card rendering in `handlers.rs`**

- [ ] **Step 4: Run tests to verify they pass**
  Run: `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && rtk cargo nextest run --workspace`
  Expected: PASS.

- [ ] **Step 5: Commit**
  Run: `rtk git add Cargo.toml Cargo.lock crates/ && rtk git commit -m "feat(mdo): parse .mdo XML metadata, configuration subsystems, and lazy L2 object cards (D7, D8)"`

---

### Task 10: Гейт `context-audit` для вехи v0.4 (`milestone_decisions.md` §3)

**Files:**
- Modify: `tests/regression_benchmark.rs`
- Modify: `reports/BENCHMARK_REPORT.md`, `reports/benchmark_latest.json`

**Interfaces:**
- Adds v0.4 metrics to `tests/regression_benchmark.rs`:
  - `bsl` compression ratio (`>= 60.0%` token reduction).
  - `scan_project` + `check_card_staleness` latency on a large synthetic repository fixture.

- [ ] **Step 1: Add v0.4 benchmark assertions to `tests/regression_benchmark.rs`**
- [ ] **Step 2: Run `CONTEXT_INIT_WRITE_BENCHMARK=1 rtk cargo test --test regression_benchmark -- --nocapture`**
- [ ] **Step 3: Update repo knowledge base (`rtk cargo run -- update .`) and commit**
  Run: `rtk git add tests/regression_benchmark.rs reports/ .context-init/ AGENTS.md && rtk git commit -m "test(audit): add v0.4 context-audit metrics (bsl compression, scan and staleness latency)"`

---

## Этап 3: Веха v0.5 «Контекст задачи и Workspace»

### Task 11: Двухуровневый дайджест задачи `context_task` (L1/L2) и семантический дифф `.mdo`/методов (`D4`, `D5`)

**Files:**
- Create: `crates/ci-core/src/analysis/task.rs`
- Modify: `crates/ci-mcp/src/tools.rs`, `crates/ci-mcp/src/handlers.rs`, `crates/ci-mcp/src/server.rs`
- Create: `src/cli/task.rs`
- Modify: `src/cli/mod.rs`
- Test: `crates/ci-core/src/analysis/task.rs`, `crates/ci-mcp/src/lib.rs`, `tests/cli.rs`

**Interfaces:**
- Produces in `ci_core::analysis::task`:
  ```rust
  pub struct TaskDigestRequest<'a> {
      pub paths: &'a [String],
      pub base_ref: Option<&'a str>,
      pub target: Option<&'a str>, // None = L1 overview digest, Some(method_or_object) = L2 hunk
      pub max_tokens: usize,
  }
  pub fn build_task_digest(
      root: &Path,
      git: &dyn GitBackend,
      index: Option<&L0Index>,
      req: &TaskDigestRequest,
  ) -> Result<String>;
  ```
- Updates `McpTaskParams` in `crates/ci-mcp/src/tools.rs` with `pub target: Option<String>` (for L2 hunk drill-down).
- Adds `context-init task [PATH] [--base <REF>] [--paths <PATH>...] [--target <SYMBOL_OR_OBJECT>]` CLI command.

- [ ] **Step 1: Write the failing tests**
  - `task_digest_l1_includes_commits_md_artifacts_mdo_semantic_diff_and_changed_methods`: in a temp git repo with a base commit and feature branch modifying a `.md` spec, adding an attribute to an `.mdo` file, and modifying a `.rs`/`.bsl` function, verify L1 output lists the commit log since `merge-base`, the markdown artifact, the semantic `.mdo` attribute diff (`+Attribute: Status`), and the modified function name.
  - `task_digest_l2_returns_specific_method_or_object_hunk`: call with `target = Some("process_order")` → returns only the diff hunk of `process_order`.

- [ ] **Step 2: Run tests to verify they fail**
  Run: `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && rtk cargo nextest run -p ci-core -p ci-mcp`
  Expected: FAIL.

- [ ] **Step 3: Implement `build_task_digest` (L1/L2), `.mdo` semantic diff, method diff detector, and CLI/MCP wiring**

- [ ] **Step 4: Run tests to verify they pass**
  Run: `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && rtk cargo nextest run --workspace`
  Expected: PASS.

- [ ] **Step 5: Commit**
  Run: `rtk git add crates/ src/ tests/ && rtk git commit -m "feat(task): two-level L1/L2 task digest with semantic .mdo diff and changed method detection (D4, D5)"`

---

### Task 12: Мультирепозиторный `context_workspace(path)` и межпроектный граф связей (`D6`)

**Files:**
- Create: `crates/ci-core/src/analysis/workspace.rs`
- Modify: `crates/ci-mcp/src/tools.rs`, `crates/ci-mcp/src/handlers.rs`, `crates/ci-mcp/src/server.rs`
- Create: `src/cli/workspace.rs`
- Modify: `src/cli/mod.rs`
- Test: `crates/ci-core/src/analysis/workspace.rs`, `crates/ci-mcp/src/lib.rs`, `tests/cli.rs`

**Interfaces:**
- Produces in `ci_core::analysis::workspace`:
  ```rust
  pub fn analyze_workspace(workspace_root: &Path, git: &dyn GitBackend) -> Result<WorkspaceReport>;
  pub fn render_workspace_l0(report: &WorkspaceReport) -> String;
  ```
- Adds MCP tool `context_workspace` (`McpWorkspaceParams { path: Option<String>, cursor: Option<String> }`) in `crates/ci-mcp/src/server.rs` and CLI subcommand `context-init workspace [PATH]`.
- Enhances `ContextInitServer::resolve_project_root` so passing `project = Some("repo-b")` resolves `<workspace_root>/repo-b` across all MCP tools (`D6`).

- [ ] **Step 1: Write the failing tests**
  - `workspace_discovers_child_git_repos_branches_dirty_counts_and_shared_symbols`: create temp parent dir with two git repos (`billing-service` and `gateway-service`) sharing symbol/endpoint `OrderDto`; run `analyze_workspace` → lists both repos with branch names, dirty file counts, and cross-project link `OrderDto` between `billing-service` and `gateway-service`.
  - `mcp_project_parameter_resolves_child_repo_by_name`: register workspace root in `ContextInitServer`, call `context_get_subsystems` with `project: Some("billing-service")` → resolves `<workspace>/billing-service`.

- [ ] **Step 2: Run tests to verify they fail**
  Run: `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && rtk cargo nextest run -p ci-core -p ci-mcp`
  Expected: FAIL.

- [ ] **Step 3: Implement `analyze_workspace`, `context_workspace` MCP tool, CLI subcommand, and child project name resolution**

- [ ] **Step 4: Run tests to verify they pass**
  Run: `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && rtk cargo nextest run --workspace`
  Expected: PASS.

- [ ] **Step 5: Commit**
  Run: `rtk git add crates/ src/ tests/ && rtk git commit -m "feat(workspace): add context_workspace multi-repo discovery, cross-project links, and project addressing (D6)"`

---

### Task 13: Гейт `context-audit` для вехи v0.5 (`milestone_decisions.md` §3)

**Files:**
- Modify: `tests/regression_benchmark.rs`
- Modify: `reports/BENCHMARK_REPORT.md`, `reports/benchmark_latest.json`

- [ ] **Step 1: Add v0.5 benchmark assertions to `tests/regression_benchmark.rs`**
  - Assert `build_task_digest` L1 token count is `< 50%` of the raw `git diff` token count on a multi-file commit.
  - Assert `analyze_workspace` across 5 synthetic git repositories completes in `< 100.0 ms`.
- [ ] **Step 2: Run `CONTEXT_INIT_WRITE_BENCHMARK=1 rtk cargo test --test regression_benchmark -- --nocapture`**
- [ ] **Step 3: Update repo knowledge base (`rtk cargo run -- update .`) and commit**
  Run: `rtk git add tests/regression_benchmark.rs reports/ .context-init/ AGENTS.md && rtk git commit -m "test(audit): add v0.5 context-audit metrics (task digest token savings and workspace-L0 latency)"`

---

## Этап 4: Веха v0.6 «Полнотекстовый BM25 и гибридный поиск»

### Task 14: Трейт `Retriever`, индекс `tantivy` BM25 с русским стеммингом и ролью `search` (`D18`)

**Files:**
- Modify: `Cargo.toml`, `crates/ci-core/Cargo.toml` (add `tantivy = "0.22"` and `rust-stemmers = "1.2"` under feature `search`, default-enabled)
- Create: `crates/ci-core/src/search.rs`
- Modify: `crates/ci-core/src/lib.rs`
- Modify: `crates/ci-mcp/src/handlers.rs:820-878`, `crates/ci-mcp/src/server.rs`
- Create: `src/cli/search.rs`
- Modify: `src/cli/mod.rs`
- Test: `crates/ci-core/src/search.rs`, `crates/ci-mcp/src/lib.rs`, `tests/cli.rs`

**Interfaces:**
- Produces in `ci_core::search`:
  ```rust
  #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
  pub struct SearchHit {
      pub subsystem_id: String,
      pub name: String,
      pub score: f32,
      pub summary: String,
      pub matched_symbols: Vec<String>,
  }

  pub trait Retriever: Send + Sync {
      fn search(&self, query: &str, top_k: usize) -> Result<Vec<SearchHit>>;
  }

  pub struct Bm25Retriever { /* tantivy index */ }
  impl Bm25Retriever {
      pub fn build(index: &L0Index, knowledge_dir: &Path) -> Result<Self>;
  }
  ```
- In `ContextHandler::search`: builds `Bm25Retriever`, optionally expands/reranks top-N via `RoleRouter` (`RoleName::Search`) when `[roles] search` is not `["none"]`, and paginates within `max_response_tokens`.
- Adds CLI command `context-init search <QUERY> [PATH] [--top-k <N>]`.

- [ ] **Step 1: Write the failing tests**
  - `bm25_retriever_ranks_russian_stemmed_and_symbol_queries`: index 5 subsystems with Russian descriptions («Проведение документов расчёта себестоимости», «Аутентификация и сессии пользователей») and exported symbols (`РассчитатьСебестоимость`, `CheckAuthToken`); query `"расчет себестоимость документ"` (different inflection) ranks the calculation subsystem #1; query `"CheckAuthToken"` ranks auth #1.
  - `search_with_llm_role_reranks_top_hits`: configure a scripted `RoleName::Search` provider and verify `ContextHandler::search` applies reranking and falls back cleanly to BM25 order if the provider fails.

- [ ] **Step 2: Run tests to verify they fail**
  Run: `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && rtk cargo nextest run -p ci-core -p ci-mcp`
  Expected: FAIL.

- [ ] **Step 3: Implement `Retriever`, `Bm25Retriever` with Russian/English stemming tokenizer, `RoleName::Search` reranking, and CLI `search` subcommand**

- [ ] **Step 4: Run tests to verify they pass**
  Run: `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && rtk cargo nextest run --workspace`
  Expected: PASS.

- [ ] **Step 5: Commit**
  Run: `rtk git add Cargo.toml Cargo.lock crates/ src/ tests/ && rtk git commit -m "feat(search): add tantivy BM25 retriever with Russian stemming and search role reranking (D18)"`

---

### Task 15: Гейт `context-audit` для вехи v0.6 и финальная синхронизация базы знаний (`milestone_decisions.md` §3)

**Files:**
- Modify: `tests/regression_benchmark.rs`
- Modify: `reports/BENCHMARK_REPORT.md`, `reports/benchmark_latest.json`
- Modify: `.context-init/knowledge/**`, `AGENTS.md`, `CHANGELOG.md`

- [ ] **Step 1: Add v0.6 benchmark assertions to `tests/regression_benchmark.rs`**
  - Measure `Bm25Retriever::search` latency (`< 20.0 ms`) and verify `recall@5 >= 0.9` on a benchmark suite of 10 Russian and English queries against the repository knowledge base.
- [ ] **Step 2: Run `CONTEXT_INIT_WRITE_BENCHMARK=1 rtk cargo test --test regression_benchmark -- --nocapture`**
- [ ] **Step 3: Update `.context-init/knowledge/`, `AGENTS.md`, and `CHANGELOG.md`**
  Run: `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && rtk cargo run -- update . && rtk cargo run -- check . --exit-code --compact`
- [ ] **Step 4: Run complete verification gate**
  Run: `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && rtk cargo fmt --all -- --check && rtk cargo clippy --workspace --all-targets -- -D warnings && rtk cargo nextest run --workspace && rtk rg -i '1[cс]' crates src tests`
  Expected: fmt/clippy/nextest exit 0, `rg` exits 1 (0 matches).
- [ ] **Step 5: Commit**
  Run: `rtk git add tests/regression_benchmark.rs reports/ .context-init/ AGENTS.md CHANGELOG.md && rtk git commit -m "test(audit): add v0.6 search latency and recall@k metrics, finalize v0.3-v0.6 roadmap"`
