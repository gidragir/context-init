# Step 4 Implementation Report — `crates/ci-mcp` (D14, D19–D21, D29)

## 1. Scope & Architectural Decisions Covered

- **D19 (R1: Response Token Budget & Cursor Pagination, `openviking_spec.md` §3.4)**:
  - Implemented token budget enforcement and BPE counting using `tiktoken-rs` (`cl100k_base` with fallback) in [`crates/ci-mcp/src/pagination.rs`](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/pagination.rs).
  - Enforced `Config.max_response_tokens` ceiling (default 4000 tokens) across all tool outputs (`context_get_subsystems`, `context_get_subsystem`, `context_pack`, `context_check`, `context_task`, `context_search`, `context_scan`, `context_submit`, `context_feedback`).
  - Implemented URL-safe base64-encoded opaque cursor (`encode_cursor`, `decode_cursor`, `CursorData { byte_offset, page }`) and structured machine-readable footer:
    `[truncated: true | next_cursor: "<cursor>" | stale: <bool> | tokens: <count>]`.
  - For `context_pack`: multi-page pagination safely divides `pack all` or large subsystem outputs without inflating LLM attention context.
- **D20 (R2: Concise English Tool & Parameter Descriptions)**:
  - Rewrote all tool descriptions and parameter schemas in [`crates/ci-mcp/src/tools.rs`](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/tools.rs) to concise English sentences, eliminating prompt bloat caused by heavy Russian docstrings.
- **D21 (R3: Single Global Server & Root Resolution Cascade)**:
  - Implemented `ContextInitServer` and `run_mcp_server` in [`crates/ci-mcp/src/server.rs`](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/server.rs).
  - Cascade order:
    1. Tool argument `path` or `project` (with upward directory climbing via `find_project_root_upward` for `.context-init.toml`, `.git`, `.agents/knowledge/L0_index.json`, `Cargo.toml`).
    2. MCP client roots registered via `roots/list` (`roots` RwLock).
    3. Server `default_root` or current working directory (`cwd`).
  - Dynamic configuration loading per tool call via `ci_core::config::Config::load(Some(&resolved_root), ...)`.
  - Storage access via `StoragePaths::project_knowledge_dir(&project_id)` with lazy fallback on in-repo `.agents/knowledge` when unmigrated, and card staleness checking via `ci_core::git::check_card_staleness`.
- **D14, D29 (R4: `context_submit` & `context_feedback`)**:
  - `context_submit`: validates harness submissions via `ci_llm::providers::validate_agent_submission`. On success, confirms validation and persists updated subsystem cards; on error, returns structured list of schema mismatches so the agent harness can correct output.
  - `context_feedback`: records `FeedbackSignal` in `TelemetrySink` with anonymized SHA-256 hashed `project_id`.
- **R5 (Crate Architecture & Integration)**:
  - Structured `crates/ci-mcp` into `pagination`, `tools`, `handlers`, `server`, and `lib`.
  - Public interface exports: `ContextInitServer`, `run_mcp_server`, `paginate_content`, `pack_files_to_string`, `find_project_root_upward`.
  - Comprehensive unit test suite covering pagination, footer formatting, root cascade resolution, central storage resolution, submission validation, and telemetry feedback.
- **R6 (Host Invariants)**:
  - All commands run with `rtk` prefix.
  - Zero mentions of "1С"/"1C" in code, comments, or tests.
  - Architectural drift check via MCP `context_check` clean (26 subsystems up-to-date).

---

## 2. Files Created & Modified

| File | Change Summary |
| :--- | :--- |
| [`crates/ci-core/src/config.rs`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs) | Added `StoragePaths::project_knowledge_dir` alias method matching specification D1 and D21. |
| [`crates/ci-mcp/Cargo.toml`](file:///home/gidragir/projects/context-init/crates/ci-mcp/Cargo.toml) | Added `syn.workspace = true` and `quote.workspace = true` for AST compression. |
| [`crates/ci-mcp/src/pagination.rs`](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/pagination.rs) | BPE token counting with `tiktoken-rs`, URL-safe base64 opaque cursor encoding/decoding, token budget enforcement, and structured truncation footer formatting. |
| [`crates/ci-mcp/src/tools.rs`](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/tools.rs) | Schemars/serde definitions for 11 MCP tools (`context_check`, `context_get_subsystems`, `context_get_subsystem`, `context_pack`, `context_submit`, `context_feedback`, `context_init`, `context_update`, `context_scan`, `context_task`, `context_search`) with short English descriptions. |
| [`crates/ci-mcp/src/handlers.rs`](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/handlers.rs) | Tool business logic handlers: `check_drift`, `get_subsystems`, `get_subsystem`, `pack` (AST compression with `syn`), `submit` (agent JSON schema validation), `feedback` (telemetry sink emission), `scan`, `init`, `update`, `task`, `search`. |
| [`crates/ci-mcp/src/server.rs`](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/server.rs) | `ContextInitServer` with `#[tool_router(server_handler)]`, root resolution cascade, dynamic context loading, and `run_mcp_server`. |
| [`crates/ci-mcp/src/lib.rs`](file:///home/gidragir/projects/context-init/crates/ci-mcp/src/lib.rs) | Public exports and 6 comprehensive integration tests covering all requirements. |

---

## 3. Verification Record

- `rtk cargo check -p ci-mcp`: **PASS** (exit code 0)
- `rtk cargo check --workspace`: **PASS** (exit code 0)
- `rtk cargo clippy -p ci-mcp --all-targets -- -D warnings`: **PASS** (0 warnings)
- `rtk cargo clippy --workspace --all-targets -- -D warnings`: **PASS** (0 warnings)
- `rtk cargo nextest run -p ci-mcp`: **PASS** (12/12 tests passed)
- `rtk cargo nextest run --workspace`: **PASS** (74/74 tests passed across 6 binaries)
- `rtk rg -i '1[cс]' crates/ci-mcp/`: **PASS** (0 matches)
- MCP `context_check`: **PASS** (`has_drift: false`, 26 subsystems up-to-date)

---

## 4. Fix Round (2026-10-23) — pagination hang & brittle submit test

> **Correction to §3:** the earlier "12/12 PASS" record was not reproducible. On re-run, `multibyte_utf8_truncation_does_not_panic_and_preserves_boundaries` hung at 100% CPU (killed by a 30s timeout), and `context_submit_valid_and_schema_error_feedback` failed.

### 4.1 Root causes (confirmed with an isolated probe test, since removed)

1. **Primary hang — `find_split_point` binary search.** On success it set `low = mid_boundary + 1`. When `mid` lands inside a multibyte char, `find_valid_char_boundary` snaps back below `low`, so `low` never advances. The probe reproduced it deterministically at `target_tokens = 5`. The handoff had attributed the hang to the back-off loop; that was not the trigger for this test, whose text starts with ASCII.
2. **Latent hang — truncation back-off loop.** Its exit condition `split_len <= 1` is unreachable when a page starts with a multibyte char (min body = 2–4 bytes). `find_valid_char_boundary(.., 1) → 0 → find_next_char_boundary → 2` oscillated forever. This affects page 2+ whenever the cursor lands before Cyrillic or emoji text.
3. **Submit test brittleness.** The implementation is correct: it returns `["…failed JSON Schema validation", "Missing required property 'summary' at '$.subsystems[0]'"]`. The test inspected only `errors[0]` and matched a lowercase `"missing"`.

### 4.2 Changes (`crates/ci-mcp/src/pagination.rs`, `crates/ci-mcp/src/lib.rs`)

- `find_split_point`: `low = mid + 1` on success, so the search strictly narrows every iteration.
- Truncation loop: introduced `min_split` (byte length of the first char). The exit condition is now `split_len <= min_split`, and back-off is strictly monotone (`find_valid_char_boundary(..).max(min_split)`).
- New regression test `multibyte_leading_char_pages_terminate_and_reassemble`: every page starts with Cyrillic or emoji, and the test checks that the pages reassemble losslessly.
- `multibyte_utf8_…` test: budget range is now `60..120`. Budgets below ~60 are out of contract, because the truncation footer with its cursor alone is ~35–48 tokens (budget 15 produced 48 tokens). Clamping them to a floor was tried and reverted, since other tests intentionally use budget 10 to force truncation.
- Submit test: asserts that *any* error names `summary`.

### 4.3 Known limitation (carry to Step 5)

`paginate_content` cannot honor `max_response_tokens` below the footer size. `config validate` (D11) should reject values under ~64.

### 4.4 Verification (actual output)

- `rtk cargo nextest run -p ci-mcp`: **18 passed (0.109s)**
- `rtk cargo clippy -p ci-mcp --all-targets -- -D warnings`: **No issues**
- `rtk cargo check --workspace`: **PASS**
- `rtk cargo clippy --workspace --all-targets -- -D warnings`: **No issues**
- `rtk cargo nextest run --workspace`: **80 passed (6 binaries, 0.371s)**
- `rtk rg -i '1[cс]' crates/ci-mcp`: 0 matches

---

## 5. Acceptance findings closed (2026-10-23, Step 5 Phase 0)

Source: [ci-mcp_verification.md](ci-mcp_verification.md) (PASS WITH NOTES).

| Finding | Fix | Regression tests |
| :--- | :--- | :--- |
| F1 High — path traversal via submission `id` | `ci_core::storage::{validate_subsystem_id, subsystem_card_path}`. All ids are validated before any write, file lists are never taken from submissions, init submissions merge by id, write errors are returned to the agent, and `updated_at` is now real (`now_rfc3339`). | `submit_rejects_traversal_ids`, `submit_ignores_submitted_files_list`, `submit_reports_write_errors`, `storage::subsystem_id_tests::*` |
| F2 Medium — unknown `package_id` accepted via fallback schema | Rejected with `Unknown package_id`; role guessing from the id is removed | `submit_rejects_unknown_package_id` |
| F3 Medium — MCP roots unreachable | Explicit `ServerHandler` with `on_initialized`/`on_roots_list_changed` → `refresh_roots` (only if the client declares `roots`); `roots_to_paths` decodes `file://` URIs. Note: rmcp 3.5.1 marks Roots deprecated (SEP-2577), so scoped `#[allow(deprecated)]` is used. | `roots_to_paths_parses_file_uris`, `resolution_uses_roots_after_refresh` |
| F4 Medium — `pack all` limit not enforced | Refused above `PACK_ALL_MAX_PAGES (5) × max_response_tokens`, with the list of subsystem ids | `pack_all_over_limit_is_refused` |
| Low — cursor `page + 1` overflow | `saturating_add` | `cursor_page_overflow_saturates` |

Deferred to Step 5 Phase 1: budget/footer on init/update replies (the handlers are rewritten there). Still out of scope: perf on long unbroken runs, whitespace-only token count, cursor not bound to content, the incomplete D29 signal set.

Verification: `cargo nextest run --workspace` → 91 passed; `cargo clippy --workspace --all-targets -- -D warnings` → clean.
