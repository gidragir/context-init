# Repo Storage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The L0–L2 knowledge base lives in `<repo>/.context-init/knowledge/` by default, is committed and shared by the team, and only changes on meaningful (structural) changes.

**Architecture:** A `storage = "repo" | "central"` config key (default `repo`) selects the knowledge dir; the resolver no longer guesses from files on disk. All legacy `.agents/knowledge` support is deleted. The L0/L1 format drops machine- and size-dependent fields (schema v3), the generator writes a file only when its bytes change, and `migrate --to` copies a base between the two modes and records the mode in `.context-init.toml`.

**Tech Stack:** Rust workspace (`ci-core`, `ci-llm`, `ci-mcp`, bin `context-init`), serde/serde_json, toml, toml_edit (bin), clap, cargo-nextest.

**Spec:** `docs/superpowers/specs/2026-10-24-repo-storage-design.md`

## Global Constraints

- Before cargo: `export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH"`; prefix shell commands with `rtk` (`rtk cargo …`, `rtk git …`); use `fd`/`rg`, never find/grep.
- Every test that creates a knowledge base sets `XDG_DATA_HOME` (and `HOME`/`XDG_CONFIG_HOME` where the existing helpers do) to a temp dir.
- No «1С»/«1C» anywhere in code, comments, tests.
- Never read `.agents/knowledge/**` directly (it gets deleted in Task 7 via `git rm`).
- Commit messages end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Work on branch `feat/repo-storage`; never push/merge.
- Repo-mode dir: exactly `<root>/.context-init/knowledge/`. Config key: `storage`, values `"repo"` | `"central"`, default `repo`. Env: `CONTEXT_INIT_STORAGE`.
- `CURRENT_L0_SCHEMA_VERSION = 3`. Error for other versions: `knowledge base uses an outdated format (schema v{N}); rebuild it with \`context-init init --force\``.
- Done gate: `rtk cargo nextest run --workspace` and `rtk cargo clippy --workspace --all-targets -- -D warnings` clean.

## Review Focus

1. Running `update` twice on an unchanged tree must leave every KB file and `AGENTS.md` byte- and mtime-identical — test in Task 4.
2. Editing a source file's body (no new/removed files) must not change any KB file — test in Task 4.
3. A file growing past another must not reorder L1 tables; the 15-file cap selection may still flip at its boundary (accepted) — test in Task 3.
4. `migrate --to` on a project with an existing `.context-init.toml` containing comments and other keys must keep them intact — test in Task 5.
5. A clone at a different absolute path must answer `pack`/`check` without `init` — e2e test in Task 3.

---

### Task 1: `storage` config key

**Files:**
- Modify: `crates/ci-core/src/config.rs` (`Config`, `PartialConfig`, `apply_toml_str`, `apply_env`, `Default`)
- Modify: `src/cli/config.rs` (`config show`/`path` print `storage: <mode>`)
- Test: `crates/ci-core/src/config.rs` tests module

**Interfaces:**
- Produces: `pub enum StorageMode { Repo, Central }` in `ci_core::config` — `#[serde(rename_all = "lowercase")]`, `Default = Repo`, `Copy`, `JsonSchema`, `pub fn as_str(self) -> &'static str` (`"repo"`/`"central"`), `FromStr` (unknown value → error naming the value and the allowed ones). Field `Config.storage: StorageMode` (`#[serde(default)]`). Re-export from `ci_core` root next to `Config`.

- [ ] **Step 1: Failing tests** — `storage_defaults_to_repo` (`Config::default().storage == StorageMode::Repo`); `storage_from_project_toml` (`apply_toml_str("storage = \"central\"")` → `Central`); `storage_env_overrides_file` (file `central`, env `CONTEXT_INIT_STORAGE=repo` via the existing `get_env` injection → `Repo`); `storage_invalid_value_errors` (`storage = "cloud"` → `Err`, message contains `cloud`; env `CONTEXT_INIT_STORAGE=cloud` → `Err`).
- [ ] **Step 2:** `rtk cargo nextest run -p ci-core storage_` → FAIL (no `StorageMode`).
- [ ] **Step 3:** Implement `StorageMode` and wire it through `PartialConfig.storage: Option<StorageMode>`, `apply_toml_str`, `apply_env` (`non_empty("CONTEXT_INIT_STORAGE")` → `FromStr`). Update the `Config` doc comment `touch_agents_md` "(D1)" reference only if wording mentions central storage. `config show` prints `storage = "<mode>"` as part of the serialized config (automatic via serde); `config path` prints `storage: <mode>` above `knowledge_dir:`.
- [ ] **Step 4:** `rtk cargo nextest run -p ci-core` → PASS.
- [ ] **Step 5:** Commit `feat(config): storage = repo|central key (default repo)`.

### Task 2: Resolver by mode, legacy removal, mismatch hint

**Files:**
- Modify: `crates/ci-core/src/analysis/mod.rs` (resolver, tests)
- Modify: `crates/ci-core/src/storage.rs` (delete `migrate_in_repo_knowledge*`, `legacy_source_dir`, `copy_missing_files`, `migrate_knowledge`, `MigrateOptions`, `MigrationReport`; keep `list_files_recursive` if Task 5 reuses it)
- Modify: `crates/ci-core/src/lib.rs` (re-exports; delete `legacy_l0_index_compatibility_and_migrate_in_repo_knowledge` test)
- Modify: `crates/ci-mcp/src/handlers.rs`, `crates/ci-mcp/src/server.rs:160-170`, `crates/ci-mcp/src/pipeline.rs`, `crates/ci-mcp/src/lib.rs` tests
- Modify: `src/cli/check.rs`, `src/cli/init_update.rs`, `src/cli/pack.rs`, `src/cli/migrate.rs` (its legacy body goes; until Task 5, `run` is `bail!("migrate is being reworked")` so the CLI compiles)
- Test: `tests/cli.rs` (delete `cli_check_reads_legacy_in_repo_knowledge`, `cli_init_on_legacy_carries_over_hand_written_files`, legacy `cli_migrate_*`; rename `cli_init_writes_central_storage`)

**Interfaces:**
- Consumes: `StorageMode` (Task 1).
- Produces:
  - `pub fn resolve_knowledge_dir(root: &Path, mode: StorageMode, central_kb: &Path) -> PathBuf` — `Repo` → `root/.context-init/knowledge`, `Central` → `central_kb`. `KnowledgeLocation` is deleted (callers use `StorageMode`).
  - `pub const REPO_KNOWLEDGE_DIR: &str = ".context-init/knowledge";` in `ci_core::analysis`.
  - `ResolvedProject { root, config, project_id, storage_paths, knowledge_dir, storage: StorageMode }` (`is_central` removed); `pub fn knowledge_dir_for(&self, mode: StorageMode) -> PathBuf`.
  - `pipeline::storage_mismatch_hint(project: &ResolvedProject) -> Option<String>` — `Some` iff `project.knowledge_dir` has no `L0_index.json` and `knowledge_dir_for(other)` has one. Text: `Knowledge base found at {other_dir}, but storage = "{mode}". Run \`context-init migrate --to {other}\` or set storage = "{other}" in .context-init.toml.` Replaces `legacy_hint` and `carry_over_notice` (both deleted, with `PipelineOutcome.carried_over`).

- [ ] **Step 1: Failing tests** — `resolve_knowledge_dir_follows_mode` (Repo → `root/.context-init/knowledge` even when `central_kb/L0_index.json` exists; Central → `central_kb` even when the repo dir has an index); in `ci-mcp` lib tests replace `central_storage_and_lazy_fallback_resolution` with `resolved_project_uses_configured_storage` (no config → `storage == Repo`, `knowledge_dir` under root; `.context-init.toml` with `storage = "central"` → under `XDG_DATA_HOME`) and `storage_mismatch_hint_points_to_other_mode` (index only in the central dir, mode Repo → hint contains `migrate --to central`; index in active dir → `None`). E2E: `cli_init_writes_repo_storage_by_default` (`init . --compact` → `project/.context-init/knowledge/L0_index.json` exists, `central_indexes(data_home)` empty, no `.agents`); `cli_init_central_mode_writes_central` (fixture `.context-init.toml` `storage = "central"` → one central index, no repo dir); `cli_check_hints_storage_mismatch` (init with central config, then delete the toml → `check .` output contains `migrate --to central`).
- [ ] **Step 2:** Run `rtk cargo nextest run --workspace` → the new tests FAIL to compile/assert.
- [ ] **Step 3:** Implement: resolver; `ResolvedProject::with_config` uses `config.storage`; `run_init` writes to `project.knowledge_dir` (no longer hard-coded central); delete the legacy code listed above and every caller; print `storage_mismatch_hint` in `check`, `pack`, `init`/`update` (CLI and MCP `context_init`/`context_update` responses) and in MCP `context_get_subsystems` when the index is missing. Fix existing tests that assumed central default by writing `storage = "central"` into their fixture toml or switching their assertions to the repo dir — whichever matches what the test is about.
- [ ] **Step 4:** `rtk cargo nextest run --workspace` → PASS; `rtk rg -n "agents/knowledge|LegacyInRepo|legacy_hint|carry_over|is_central" crates src tests` → no hits.
- [ ] **Step 5:** Commit `feat(storage)!: resolve knowledge dir from storage mode, drop legacy .agents/knowledge`.

### Task 3: Schema v3 without volatile fields

**Files:**
- Modify: `crates/ci-core/src/storage.rs` (`L0Index`, `SubsystemFile`, `SubsystemInfo`, version constants, `from_json_str`; delete `is_legacy`, `upgrade_to_current`, `LEGACY_L0_SCHEMA_VERSION`, `default_legacy_schema_version`)
- Modify: `crates/ci-core/src/analysis/generator.rs` (`render_l0_markdown`, `render_subsystem_card`, `generate` L0 literal, `update` legacy upgrade call)
- Modify: `crates/ci-core/src/analysis/detector.rs:896-911` (`representative_files`)
- Modify: `crates/ci-core/src/analysis/subsystems.rs:101` (`compute_subsystem_tokens`)
- Modify: `crates/ci-mcp/src/handlers.rs:280-290` (synthesized card)
- Test: storage/generator/detector tests, `crates/ci-mcp/src/lib.rs`, `tests/cli.rs`

**Interfaces:**
- Produces:
  - `L0Index` without `root_path`; `schema_version: u32` required (no serde default). `from_json_str` errors with the Global Constraints message when `schema_version != CURRENT_L0_SCHEMA_VERSION` (missing field → same message with `v1`).
  - `SubsystemFile.lines` and `SubsystemInfo.tokens` stay in memory but carry `#[serde(skip)]` (`#[schemars(skip)]` too).
  - `pub fn subsystem_tokens(root_dir: &Path, sub: &SubsystemInfo) -> usize` in `ci_core::analysis::subsystems` (replaces private `compute_subsystem_tokens`; `build_deterministic` still fills `tokens` with it for split decisions).
  - `representative_files`: select the 15 by (entrypoint first, lines desc, path asc), then return them ordered entrypoints first, then by `path` asc.

- [ ] **Step 1: Failing tests** — `l0_v3_omits_volatile_fields` (generate into temp KB; `L0_index.json` text contains none of `"root_path"`, `"lines"`, `"tokens"`; `schema_version` == 3; `L0_index.md` and every `subsystems/*.md` contain neither `Строк` nor `токенов` nor `Токены`); `l0_old_schema_is_rejected` (`schema_version: 2` JSON → `Err` containing `schema v2` and `init --force`); `representative_files_sorted_by_path` (inputs b.rs 10 lines, a.rs 500 lines, entry main.rs → `[main.rs, a.rs, b.rs]`; then make b.rs 900 lines → same order); MCP `synthesized_card_counts_tokens_live` (index without card file; write a source file; card text contains `~N` with `N == subsystem_tokens(...)` and `N > 0`). E2E `cli_clone_at_other_path_works_without_init` (init in sandbox project, copy the whole project dir to `<sandbox>/clone`, run `pack <some id> --compress` and `check .` in the clone → both succeed, output has no `init` hint).
- [ ] **Step 2:** `rtk cargo nextest run --workspace` → FAIL.
- [ ] **Step 3:** Implement. L0 table header becomes `| Подсистема | Роль / Назначение | Точки входа | L1 Карточка |`; L1 file table `| Файл | Статус |`; L1 header line without the `**Объем:**` part. Synthesized MCP card keeps `**Tokens:** ~{}` computed with `subsystem_tokens(&project.root, sub)`.
- [ ] **Step 4:** `rtk cargo nextest run --workspace` → PASS.
- [ ] **Step 5:** Commit `feat(storage)!: schema v3 drops root_path, line and token counters`.

### Task 4: Quiet writes, `updated_at`, mode-aware AGENTS.md

**Files:**
- Modify: `crates/ci-core/src/analysis/generator.rs` (`GenerateOptions`, `generate`, `update`, `update_or_create_root_agents_md`, new `write_if_changed`)
- Modify: `crates/ci-mcp/src/pipeline.rs` (`PipelineOptions` → pass mode), CLI/MCP callers
- Test: generator tests, `tests/cli.rs`

**Interfaces:**
- Consumes: `StorageMode` (Task 1).
- Produces:
  - `GenerateOptions { force, dry_run, agents_md: Option<StorageMode> }` — `None` = do not touch `AGENTS.md` (replaces `with_agents_md: bool`).
  - `pub fn update(analysis, new_subsystems, knowledge_dir, agents_md: Option<StorageMode>) -> Result<(usize, usize)>`.
  - `fn write_if_changed(path: &Path, content: &str) -> Result<bool>` — reads the file, writes only when bytes differ (creating parents), returns whether it wrote.

`update` algorithm (the part tests don't fully determine):
1. Build the new index and all rendered files (L1 cards, `cli/commands.md`, `L0_index.md`, `L0_index.json`) with `updated_at` = the previous value; subsystems sorted by `id`; `files`/`entrypoints`/`key_files`/`tags` of each subsystem sorted asc, `key_symbols` kept in detector order (signatures, not names).
2. Compare each rendered file and the set of superseded cards to disk. If nothing differs → write nothing, return.
3. Otherwise set `updated_at = today`, re-render `L0_index.json`/`L0_index.md`, then `write_if_changed` every file and delete superseded cards.
`generate` (init / `init --force`) writes with `updated_at = today` but still uses `write_if_changed`.

AGENTS.md section text: the sentence `которая хранится вне репозитория` is kept for `Central`; for `Repo` it becomes: `которая лежит в \`.context-init/knowledge/\` и коммитится вместе с кодом. Людям можно читать её напрямую; агентам — предпочитать MCP/\`pack\` ниже: это компактнее и учитывает бюджет токенов. После структурных изменений запустите \`context-init update\` и закоммитьте изменения базы в том же PR.` `update_or_create_root_agents_md` also goes through `write_if_changed`.

- [ ] **Step 1: Failing tests** — `update_twice_is_noop` (generate, `update` with same analysis; record bytes + mtime of every file under KB and `AGENTS.md`; sleep 1.1 s; `update` again → all identical); `body_edit_does_not_touch_kb` (fixture project, init, append a line to a source file body, re-analyze, `update` → no KB file changed); `structural_change_bumps_updated_at` (preset index `updated_at = "2000-01-01"`, add a subsystem → `updated_at` == today and the new card exists; unchanged run keeps `"2000-01-01"`); `update_keeps_enriched_summary` (index with summary `"Ручное описание"` → after `update` with deterministic subsystems summary still `"Ручное описание"`); `agents_md_wording_follows_mode` (Repo → contains `.context-init/knowledge/` and `закоммитьте`; Central → contains `вне репозитория`). E2E `cli_update_twice_leaves_git_clean` (sandbox `git init`, init, `git add -A && git commit`, `update .` → `git status --porcelain` empty).
- [ ] **Step 2:** `rtk cargo nextest run --workspace` → FAIL.
- [ ] **Step 3:** Implement per the algorithm; callers pass `opts.with_agents_md.then_some(project.storage)`.
- [ ] **Step 4:** `rtk cargo nextest run --workspace` → PASS.
- [ ] **Step 5:** Commit `feat(generator): write only on change, stable order, mode-aware AGENTS.md`.

### Task 5: `migrate --to repo|central`

**Files:**
- Modify: `crates/ci-core/src/storage.rs` (new copy function)
- Rewrite: `src/cli/migrate.rs`
- Test: storage tests, `tests/cli.rs`

**Interfaces:**
- Consumes: `ResolvedProject::knowledge_dir_for` (Task 2), `StorageMode` (Task 1).
- Produces:
  - `pub struct CopyKbOptions { pub dry_run: bool, pub remove_source: bool }`, `pub struct CopyKbReport { pub files: Vec<String>, pub subsystems_count: usize, pub removed_source: bool }`.
  - `pub fn copy_knowledge(src: &Path, dst: &Path, opts: &CopyKbOptions) -> Result<CopyKbReport>` — errors if `src/L0_index.json` is missing or fails `L0Index::load_from_file`; copies every file (relative `/` paths, sorted); verifies `dst/L0_index.json` loads; removes `src` only after that and only with `remove_source`; `dry_run` writes nothing.
  - CLI: `migrate [PATH] --to <repo|central> [--remove-source] [--dry-run] [--force]` (`--to` required, `value_parser` over `StorageMode`). Flow: source = `knowledge_dir_for(other(to))`, target = `knowledge_dir_for(to)`; if `project.storage == to` and target has an index → print `already using storage = "<to>"`, exit 0; target has index and no `--force` → error; `copy_knowledge`; unless `--dry-run`, set `storage = "<to>"` in `<root>/.context-init.toml` via `toml_edit::DocumentMut` (create the file if missing, keep other keys and comments). `fn set_storage_key(root: &Path, mode: StorageMode) -> Result<PathBuf>` lives in `src/cli/migrate.rs`. Compact output: `OK: migrated N files (M subsystems) <src> -> <dst>; storage = "<to>"`.

- [ ] **Step 1: Failing tests** — `copy_knowledge_copies_and_verifies` (src with L0 + `adr/x.md` → both in dst, report lists `L0_index.json`, `adr/x.md`); `copy_knowledge_rejects_missing_index`; `copy_knowledge_dry_run_writes_nothing`; `copy_knowledge_remove_source`. E2E `cli_migrate_round_trip` (init (repo) → `migrate . --to central` → central index exists, toml has `storage = "central"`, `pack` works → `migrate . --to repo --remove-source` → repo index exists, central dir gone, toml `storage = "repo"`); `cli_migrate_keeps_toml_comments` (toml `# keep me\nlanguage = "en"\n` → after migrate both lines present plus `storage`); `cli_migrate_refuses_existing_target_without_force`; `cli_migrate_noop_when_already_in_mode`.
- [ ] **Step 2:** `rtk cargo nextest run --workspace` → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `rtk cargo nextest run --workspace` → PASS.
- [ ] **Step 5:** Commit `feat(migrate)!: copy knowledge base between repo and central storage`.

### Task 6: Docs and decisions

**Files:**
- Modify: `README.md`, `CHEATSHEET.md`, `CHANGELOG.md`, `milestone_decisions.md`; `src/cli/init_update.rs` (repo-mode `init` hint)

- [ ] **Step 1:** In repo mode `init` (non-compact) prints: `Tip: add \`.context-init/knowledge/** linguist-generated=true\` to .gitattributes to collapse knowledge-base diffs.` — add an assertion to `cli_init_writes_repo_storage_by_default` (non-compact run) that stdout contains `linguist-generated`; run, see FAIL, implement, PASS.
- [ ] **Step 2:** README/CHEATSHEET: section «Хранение базы»: repo по умолчанию (`.context-init/knowledge/`, коммитится), `storage = "central"` опция, `migrate --to`, рабочий процесс: `context-init update` перед коммитом, база в том же PR; после merge-конфликта в базе — `context-init update`; `.gitattributes` совет. Remove every mention of `.agents/knowledge`, legacy fallback and old `migrate`.
- [ ] **Step 3:** CHANGELOG `Unreleased` → `### Breaking`: default storage repo; schema v3 (rebuild with `init --force`); legacy `.agents/knowledge` support removed; `migrate` now `--to repo|central`.
- [ ] **Step 4:** `milestone_decisions.md`: D1 row prefixed «**Пересмотрено (2026-10-24, D1a).**»; new row D1a: «База L0–L2 по умолчанию в репозитории (`<repo>/.context-init/knowledge/`, `storage = "repo"`), коммитится и общая для команды; `storage = "central"` — центральное хранилище; кэш, телеметрия, feedback всегда централизованы; формат детерминирован (schema v3 без `root_path`/счётчиков, запись только при изменениях)»; D23 prefixed «**Отменено (2026-10-24):** поддержка legacy `.agents/knowledge` удалена.»
- [ ] **Step 5:** `rtk rg -n "agents/knowledge" README.md CHEATSHEET.md` → no hits. Commit `docs: repo storage mode`.

### Task 7: Dogfood and final verification

**Files:**
- Delete: `.agents/knowledge/` (tracked) via `rtk git rm -r -q .agents/knowledge`
- Create: `.context-init/knowledge/**` (generated), `AGENTS.md` (regenerated section)

- [ ] **Step 1:** `rtk cargo build --release` then `./target/release/context-init init . --force --with-agents-md` (exact AGENTS flag per current CLI `--help`). Expect `.context-init/knowledge/L0_index.json` with `"schema_version": 3`.
- [ ] **Step 2:** `./target/release/context-init update .` → `rtk git status --porcelain .context-init AGENTS.md` shows no new changes after the first commit of the base (run update, commit, run update again, status empty).
- [ ] **Step 3:** `rtk cargo nextest run --workspace` and `rtk cargo clippy --workspace --all-targets -- -D warnings` → clean.
- [ ] **Step 4:** Commit `chore: dogfood repo storage for context-init`. The old central KB in `~/.local/share/context-init/projects/` is left untouched; report it to the user (deleting it needs their OK).
