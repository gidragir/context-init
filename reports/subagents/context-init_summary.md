# Step 5 Implementation Report: `context-init` binary over the `ci-*` crates

- **Spec:** [docs/superpowers/specs/2026-10-23-v0.3-step5-cli-design.md](../../docs/superpowers/specs/2026-10-23-v0.3-step5-cli-design.md)
- **Plan:** [docs/superpowers/plans/2026-10-23-v0.3-step5-cli.md](../../docs/superpowers/plans/2026-10-23-v0.3-step5-cli.md)
- **Branch:** `feat/v0.3-step5` (baseline `019e04b`)
- **Executor:** inline (executing-plans), TDD per task; whole-branch review at the end.

## 1. What changed

| Phase | Result |
| :--- | :--- |
| 0 — Step 4 findings | F1 path traversal in `context_submit` (ids validated, writes confined, no agent-supplied file lists). F2 unknown `package_id` rejected. F3 `roots/list` requested on `initialized` / `roots_list_changed`. F4 `pack all` limit (5 × budget). Cursor page overflow saturates. |
| 1 — single pipeline | `ci_core::analysis` {detector, generator, subsystems, pack, `resolve_knowledge_dir`}; `ci_llm::enrich` (replaces `src/ollama.rs`); `ci_mcp::pipeline` (init/update orchestration shared by CLI and MCP); MCP `context_init` performs real detection into central storage and registers agent work packages. Legacy `src/{lib,service,mcp,ollama,detector,generator,types}.rs` deleted; thin `src/cli/*`. |
| 2 — config + migrate | `config init/show/path/validate`; `Config::lint`, `MIN_RESPONSE_TOKENS = 64`, `loaded_sources`. `migrate [--remove-source] [--dry-run] [--force]` with verified removal and support for the nested legacy layout. |
| 3 — setup | `setup <antigravity|claude-code|claude-desktop|cursor|gemini-cli|codex|all> [--write]`: user-level only, dry-run diff by default, `.bak.<stamp>` backups, idempotent marker blocks, Claude Code skill, ≤15-line rule. |
| 4 — telemetry + feedback | `telemetry report [--month] [--notes] [--submit] [--yes]` (gh or prefilled URL, confirmation required). `feedback <tool> <verdict> [--note]` shares `record_feedback` with MCP; this also fixes `recorded: true` being reported while telemetry was off. |

The breaking CLI changes are documented in `README.md` (migration table) and `CHANGELOG.md`.

## 2. Verification (actual output, 2026-10-23)

- `cargo check --workspace`: OK
- `cargo clippy --workspace --all-targets -- -D warnings`: no issues
- `cargo nextest run --workspace`: **147 passed**
- `rg -i '1[cс]' crates src tests`: no product mentions
- Regression benchmark: drift audit avg 21.9 ms (baseline 28.4 ms); AST compression 90.6% on `crates/ci-core/src/analysis/detector.rs` (the baseline's 85.9% was measured on the deleted `src/service.rs`); external ai-dotfiles fixture: 6 subsystems, pack OK (1 file, ~685 tokens), identical to baseline.
- Knowledge base of this repo: `init` wrote 7 subsystems to central storage; `check . --exit-code` exits 0. The old in-repo index had 9 entries, 7 of them single files in `src/` that now live in 3 crates.
- Smoke (real HOME, read-only): `setup antigravity` dry run touches only `mcpServers.context-init` (+ GEMINI.md block); `config validate` OK; `migrate --dry-run` lists 14 files.

## 3. Known limitations / follow-ups

- **Detector granularity:** a workspace crate is a single subsystem (`crates_ci-core` ≈ 73k raw tokens). Large crates should be split per module to keep `context_pack` replies small.
- The AGENTS.md template does not add the `rtk` prefix to its CLI fallback commands.
- Not covered by tests: an end-to-end `roots/list` exchange with a live MCP client (URI conversion and cascade are tested), and the failure branch of the migrate verification guard.
- Earlier test runs (before Step 5 Task 8 isolated storage) left 144 `ci_mcp_*` / `my_repo-*` folders in `~/.local/share/context-init/projects`.
