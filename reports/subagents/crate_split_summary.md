# Splitting oversized crates into per-module subsystems

Branch `claude/detector-crate-split-679caf` (based on `feat/v0.3-step5`).

## What changed

- **Threshold.** New `Config::split_threshold_tokens`: TOML key `split_threshold_tokens`,
  env `CONTEXT_INIT_SPLIT_THRESHOLD_TOKENS`. Default `15000` BPE tokens; `0` disables
  splitting. `init`, `update`, `check` and MCP `context_check` all read it from the project config.
- **Detector** (`detector::analyze_project_with` + `AnalyzeOptions`; `analyze_project` keeps
  its signature and uses the default threshold). When a module's full source (every collected
  file, not only the 15 representative ones) is over the threshold, the module is split:
  - The entries considered are the top-level items of `<module>/src/`, plus the module's other
    top-level items (`Cargo.toml`, `tests/`, …). A file and a directory with the same stem
    (`foo.rs` + `foo/`) count as one Rust module.
  - Greedy and deterministic: the largest entries that are not entrypoints are promoted to
    child modules `<id>_<stem>` (ties broken by name) until what is left fits. What is left keeps
    the module's **original id** (`crates_ci-core`). Entrypoints (`lib.rs`, `mod.rs`,
    `Cargo.toml`, …) never leave their module.
  - A promoted directory that is still over the threshold is split the same way, recursively.
    A single file over the threshold cannot be split further.
  - Ids go through `sanitize_id`, so they match `SUBSYSTEM_ID_PATTERN`.
- **Stable ids.** A module under the threshold produces exactly the same `DiscoveredModule`
  as before (id = `sanitize_id(dir_path)`), so existing knowledge bases do not churn.
- **Parent link.** `DiscoveredModule` gains `id` and `parent`. `SubsystemInfo` gains an optional
  `parent` field, which is skipped when `None`, so old `L0_index.json` files still load.
- **Drift.**
  - Crate split: the crate keeps its id; children show up as `untracked`, and `update` adds them.
    `update` also rewrites the crate's file list, so the crate no longer packs the code that
    moved to its children.
  - Crate shrinks back under the threshold: a subsystem with a `parent` that is no longer
    detected is reported as `missing_in_fs`. `update` removes it from the index and deletes its
    L1 card (through `safe_join`).
- **Pack.** No change needed. Children carry real `files`, so `pack_subsystem` and its `safe_join`
  guards work as before.
- **Speed.** Detection now BPE-tokenizes oversized modules, and symbols are extracted for more
  modules. To keep audits fast:
  - Modules whose byte size already fits skip tokenizing (a token spans at least one byte).
  - Each file is tokenized once per run, in parallel (`std::thread::scope`).
  - Token counts and symbols are memoized per process, keyed by path, size and mtime
    (`FileMemo`). This helps the MCP server's repeated `context_check`.
- **Tests moved.** The tests in `detector.rs` moved to `detector/tests.rs` (`mod tests;`).
  Without this, `detector.rs` alone was 21,115 tokens, more than the threshold. After the move
  the split puts it in two subsystems: `…_analysis_detector` (14,884) and
  `…_analysis_detector_tests` (6,128).

## Tests (TDD: RED was a compile failure on the missing API, then GREEN)

- `detector/tests.rs`:
  - `test_crate_under_threshold_keeps_single_stable_subsystem`
  - `test_crate_over_threshold_is_split_into_module_subsystems`: ids, parent links,
    small files stay in the root, every file lands in exactly one subsystem, every subsystem is
    under the threshold, ids are valid, output is deterministic
  - `test_split_merges_file_and_dir_with_same_stem`
  - `test_split_threshold_zero_disables_splitting`
- `generator.rs`: `test_crate_split_is_clean_added_and_removed_drift`. It runs
  generate → split → audit/update → shrink back → audit/update; the card is removed at the end.
- `config.rs`: `split_threshold_defaults_and_layers`.

## Verification

- `cargo test --workspace`: all passed (ci-core 67).
- `cargo clippy --workspace --all-targets -- -D warnings`: clean. `cargo fmt --check`: clean.
- Regression benchmark (`CONTEXT_INIT_BENCHMARK_EXTERNAL_REPO=/data/projects/ai-dotfiles`):
  - Before: drift audit avg 23.08 ms; ai-dotfiles `6 subsystems, drift=true, pack OK (files=1, tokens~685)`.
  - After: drift audit avg 14.91 ms. The first, cold iteration takes ~105 ms because it loads
    the BPE tables. ai-dotfiles is **unchanged**: `6 subsystems, drift=true, pack OK (files=1, tokens~685)`.
- `context-init init .` with isolated `XDG_DATA_HOME`/`XDG_CONFIG_HOME` and `--role init=none`:
  30 subsystems, **none above 15,000 tokens** (largest 14,884), all ids valid.
  - `check` right after `init`: `OK: 30 subsystems up-to-date`.
  - Old unsplit KB checked against the split tree: `missing=[]`, 23 children `untracked`.
  - `update` on that KB: `23 added, 7 refreshed`; `check` afterwards is `OK`.
- `pack <id> --compress` works on the root and on child subsystems.
  - Largest compressed code pack: **79.6 KB → 20.7 KB**.
  - `crates_ci-core`: 79,653 → 5,063 bytes. It now holds only `lib.rs` and `Cargo.toml`.

## Subsystem sizes (BPE tokens, `L0_index.json` `tokens`)

### Before (`split_threshold_tokens = 0`, the old behaviour)

| Subsystem | Tokens |
| :--- | ---: |
| `crates_ci-core` | 80,936 |
| `crates_ci-llm` | 40,635 |
| `reports` | 39,293 |
| `crates_ci-mcp` | 28,254 |
| `src_cli` | 14,488 |
| `tests` | 10,238 |
| `references` | 939 |

### After (default 15,000)

| Subsystem | Tokens | Parent |
| :--- | ---: | :--- |
| `crates_ci-core_analysis_detector` | 14884 | `crates_ci-core_analysis` |
| `src_cli` | 14488 | — |
| `crates_ci-llm` | 13320 | — |
| `crates_ci-mcp` | 12540 | — |
| `reports_subagents` | 11540 | `reports` |
| `crates_ci-core` | 11262 | — |
| `crates_ci-core_config` | 10415 | `crates_ci-core` |
| `tests` | 10238 | — |
| `crates_ci-llm_providers` | 9712 | `crates_ci-llm` |
| `reports_subagents_ci-core_verification` | 8992 | `reports_subagents` |
| `crates_ci-core_analysis` | 8971 | `crates_ci-core` |
| `crates_ci-core_analysis_generator` | 7890 | `crates_ci-core_analysis` |
| `crates_ci-core_storage` | 7460 | `crates_ci-core` |
| `crates_ci-mcp_handlers` | 6737 | `crates_ci-mcp` |
| `reports_subagents_ci-llm_verification` | 6602 | `reports_subagents` |
| `crates_ci-core_analysis_detector_tests` | 6128 | `crates_ci-core_analysis_detector` |
| `crates_ci-core_telemetry` | 5881 | `crates_ci-core` |
| `reports_subagents_step1_workspace_verification` | 5685 | `reports_subagents` |
| `reports_subagents_ci-mcp_verification` | 5206 | `reports_subagents` |
| `crates_ci-mcp_pagination` | 4510 | `crates_ci-mcp` |
| `crates_ci-mcp_server` | 4467 | `crates_ci-mcp` |
| `crates_ci-llm_prompts` | 4368 | `crates_ci-llm` |
| `crates_ci-core_openviking` | 4170 | `crates_ci-core` |
| `crates_ci-llm_schema` | 4121 | `crates_ci-llm` |
| `crates_ci-llm_decorators` | 3786 | `crates_ci-llm` |
| `crates_ci-core_git` | 3772 | `crates_ci-core` |
| `crates_ci-llm_router` | 2835 | `crates_ci-llm` |
| `crates_ci-llm_enrich` | 2493 | `crates_ci-llm` |
| `reports` | 1268 | — |
| `references` | 939 | — |

## Notes / follow-ups

- The split applies to any detected module, not only crates. So `reports/` (markdown) is split
  too, into `reports_subagents_*`.
- A single file over the threshold stays one subsystem; a file cannot be split. Today no file
  reaches that size.
- The memo key is path, size and mtime. An edit that keeps both the size and the mtime
  (sub-second precision) would reuse the stale value. That is the usual trade-off of
  make-style caching.
- `init` rewrites the navigation map in this repo's `AGENTS.md` (now 30 subsystems). That
  change is not committed here.
