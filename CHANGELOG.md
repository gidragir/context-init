# Changelog

## [0.4.0] - 2026-11-08

### Added
- **Core L0 Compactness Projection**: `context_get_subsystems` returns a dedicated compact `L0SubsystemSummary` (id, name, summary, tokens, file_count, parent, stale) instead of the full `SubsystemInfo` with all file lists and symbols. Eliminates MCP pagination overhead and reduces JSON response size by 80.7% (from ~41.8 KB to ~8.0 KB on 36 subsystems).
- **Pruning Support (`update --prune`)**: Added `--prune` flag to `context-init update` and `prune: true` parameter to `context_update`. Automatically cleans removed root and split subsystems from `L0_index.json`, `L0_index.md`, deleted L1 card files, and `AGENTS.md`. `check` outputs actionable hints when stale subsystems remain.
- **Batch Multi-Repo Inspection (`context_workspace`)**: Added `paths: Vec<String>` support to `context_workspace` and `--repo <PATH>` to the CLI. Displays a unified `| Repository | Branch | Dirty | KB Status | Subsystems |` table with `WorkspaceKbStatus` (`Ok`, `Drift`, `LegacyAgentsDir`, `Uninitialized`, `MissingPath`) without heavy BPE tokenization. Disambiguates identical directory basenames via parent path segments.
- **Enhanced Topology Scanning (`context_scan`)**: Extracted `file_kind.rs` classifier for Dockerfile, Kubernetes, CI/CD, lockfiles, and configs. Added statistical `Outliers` detection and clean `suggested_scope` calculation. Added `max_depth` (1..=5, default: 2) aggregation and `--include-hidden` diagnostics flag.

### Fixed
- **Subsystem ID Collision Hardening**: Truncated subsystem IDs longer than 64 characters now append a deterministic 8-hex hash suffix (`_{hash}`) to avoid collisions. Duplicate subsystem IDs are pruned during L0 generation.
- **Monorepo Container Direct Files**: Subsystem discovery preserves direct files in monorepo container directories (e.g. `packages/`, `crates/`) while properly excluding child package modules without recursive duplication.
- **AST Splitter Hardening**: Enforced 1,000 token minimum floor for single-file promo subsystems and atomic flat collections. Eliminated orphaned micro-subsystems (`mise.rs`) and test module splittings.

### Removed
- **Stow-Specific Hardcoding**: Removed host-specific dotfiles/Stow special-casing in favor of declarative `[scope].include` directory traversal with prefix-matched globbing. Legacy `.context-init.toml` files with `stow = ...` remain backward-compatible.

### Added
- Adoption and integrity hardening (v0.6+):
  - **Telemetry & Efficiency Observability**: Opt-in telemetry records operational signals and token savings comparing response tokens against raw source tokens; detects client harness (`claude-code`, `antigravity`, `terminal`) and execution channel (`mcp`, `cli`); `telemetry report` aggregates usage, savings and nested tool calls; automated issue submission via `telemetry report --submit`.
  - **L0 Index Schema v4**: Provenance tracking for subsystem summaries (`heuristic`, `manual`, `llm`, `agent`), `summary_model`, `needs_review` flag for drift review, and `generator` build stamp with backward-compatible v3-to-v4 in-memory migration.
  - **Agent Submission Lifecycle**: `context_submit` enforces single-use package tokens, confines writes strictly to the active knowledge directory, and applies agent work packages across the full knowledge base.
  - **Selective Update Enrichment**: `update` enriches only new, review-flagged, or file-drifted subsystems (`subsystems_needing_enrichment`), leaving untouched manual and agent summaries intact.
  - **Binary & Format Drift Detection**: `context-init check` detects running MCP binary obsolescence and card format drift; `mise run install` performs atomic release builds to `~/.local/bin/context-init`.
  - **Detector & Search Refinements**: AST symbol extractor skips `#[cfg(test)]` items and test fixture declarations; BM25 search ranking demotes test-only subsystems; MCP tools carry clear routing hints in tool descriptions.
  - **Subagent Adoption & Transcript Auditing**: `context-init setup` adds a dedicated `### Subagents and scripts (CLI)` guidance section to agent rules; `scripts/audit-sessions.py` audits transcript adoption across Claude Code and Antigravity sessions; `scripts/probe-adoption.sh` benchmarks harness adoption against 5 standardized probes (S5 criterion).
- v0.3 hardening: one `check` pipeline for CLI and MCP (missing index is an error, Git-stale cards count as drift), strict `--model` validation, atomic and pre-planned `setup` writes, `mise.toml` tasks, CI/release workflows, `context-audit` gate.
- v0.4: L1 cards are OpenViking OKF sidecars (YAML frontmatter with `directory`, `source` SHAs, `generated_by`, `freshness`); lazy Git staleness in `context_get_subsystem` with optional `auto_refresh`; two-phase `scan`/`init` with `.gitignore` and `[scope]` (`--include`, `--exclude`, `--ext`); `LanguageAdapter` for Rust, Python, BSL and `.os`; `.mdo` metadata subsystems and lazy L2 object cards.
- v0.5: `context_task` L1/L2 digest (`--target`) with semantic `.mdo` diff and changed methods; `context_workspace` and bare-name `project` addressing across repositories.
- v0.6: tantivy BM25 search with Russian/English stemming (`context-init search`), optional `search`-role reranking, `recall@5` gate.

### Breaking
- L1 card layout: the summary follows the H1 and cards start with YAML frontmatter. Bases written by v0.3 are not migrated by `update`; rebuild them with `context-init init --force`.

### Breaking (v0.3)
- The binary is now a thin CLI over the `ci-core`, `ci-llm` and `ci-mcp` crates. `context-init [PATH]` with top-level flags is gone; use `context-init init [PATH]`. A bare `context-init` still runs `init .`.
- Removed `--ai`, `--ollama-url` and `-m/--model <ollama model>` from `init`/`update`. Configure models in `[models.<alias>]`, then select them with the global `--model <alias>` or `--role <role>=<alias>`. Deterministic mode (no LLM) is the default.
- `--with-agents-md` is replaced by `--no-agents-md` (the AGENTS.md navigation section is on by default).
- Default storage is the repository: `init` writes the knowledge base to `<repo>/.context-init/knowledge/` (`storage = "repo"`), to be committed with the code. `storage = "central"` keeps it in `~/.local/share/context-init/projects/<id>/`.
- `L0_index.json` schema v3: `root_path`, per-file `lines` and per-subsystem `tokens` are no longer stored (sizes are computed on demand); older bases must be rebuilt with `context-init init --force`.
- Support for the legacy `.agents/knowledge` layout is removed.
- `migrate` now takes `--to repo|central` and copies the knowledge base between the two storages.
- MCP `context_init`/`context_update` take `model` instead of `ai`. `context_check` reports subsystem ids in `missing_in_fs`.

### Added (v0.3)
- `update` writes only files whose bytes change and keeps a stable order, so re-running it on an unchanged tree leaves the working tree clean.
- MCP `context_init` performs real project detection (it was a placeholder) and can hand an agent work package to the harness (`kind = "agent"`).
- MCP server reads the client's `roots/list` (D21).
- Global `--config <FILE>`.

### Security
- `context_submit` validates subsystem ids, confines writes to the knowledge directory, and rejects unknown work packages.
