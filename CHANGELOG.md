# Changelog

## [Unreleased]

## [0.3.0](https://github.com/gidragir/context-init/releases/tag/v0.3.0) - 2026-10-29

### Added

- *(search)* add tantivy BM25 retriever with Russian stemming and search role reranking (D18)
- *(workspace)* add context_workspace multi-repo discovery, cross-project links, and project addressing (D6)
- *(task)* two-level L1/L2 task digest with semantic .mdo diff and changed method detection (D4, D5)
- *(mdo)* parse .mdo XML metadata, configuration subsystems, and lazy L2 object cards (D7, D8)
- *(scan)* two-phase scan/init with .gitignore walker and persistent [scope] filters (D9)
- *(kb)* emit OpenViking OKF frontmatter in L1 cards and support lazy git stale auto_refresh (D2)
- *(migrate)* [**breaking**] copy knowledge base between repo and central storage
- *(generator)* write only on change, stable order, mode-aware AGENTS.md
- *(storage)* [**breaking**] schema v3 drops root_path, line and token counters
- *(storage)* [**breaking**] resolve knowledge dir from storage mode, drop legacy .agents/knowledge
- *(config)* storage = repo|central key (default repo)
- *(detector)* split oversized crates into per-module subsystems
- *(cli)* feedback command sharing ci-core recording with MCP (D29)
- *(cli)* telemetry report and opt-in GitHub submission (D30)
- *(setup)* rule/skill blocks and setup command for six harnesses
- *(setup)* harness table and non-destructive config merge (D22)
- *(cli)* migrate in-repo knowledge to central storage (D23)
- *(cli)* config init/show/path/validate (D11)
- *(workspace)* v0.3 steps 1-4 (ci-core, ci-llm, ci-mcp) + step 5 spec/plan
- *(cli)* add check, update, pack and mcp subcommands
- *(mcp)* implement context-init MCP server tools
- *(core)* add deep module discovery, AST parsing and context service
- initial commit for context-init OpenViking CLI tool

### Fixed

- *(test)* compile test helpers cleanly on windows
- *(minors)* single Unreleased changelog, no double benchmark run, up_to_date_count, config warning, query header, mdo cap coverage
- *(final-review)* validate card shas before git, confine search card reads, keep setup backups, paginate rerank safely
- *(stage2)* digest ids for non-ASCII metadata names, validate scope globs before persisting, BSL BOM/async/unterminated methods, no full read of huge files in scan
- *(stage1)* validate --model like the router, split --model chains, write through symlinked configs, drop duplicate crates.io publish
- *(setup,telemetry)* atomic config writes, pre-plan setup all, inline TOML tables, secure temp file
- *(check,cli)* unify CLI/MCP drift check, reject unknown --model alias, add rtk fallback in AGENTS.md
- *(setup)* detect claude-code by ~/.claude for 'setup all'; cap preview diff size
- *(init)* carry over hand-written files from a legacy knowledge base; clearer migrate --force warning
- *(setup)* backups keep the original file's permissions
- *(cli)* accurate legacy knowledge-base hint for check vs init
- *(ci-mcp)* enforce pack-all budget limit (D19), saturate cursor page

### Other

- run all tests even after a failure to see the full windows picture
- *(plan)* add v0.3-v0.6 implementation plan
- *(kb)* restamp knowledge base
- *(kb)* refresh knowledge base after final-review fixes
- *(kb)* finalize v0.3-v0.6 knowledge base and changelog
- *(audit)* add v0.6 search latency and recall@k metrics
- *(kb)* refresh knowledge base after v0.5 changes
- *(audit)* add v0.5 context-audit metrics (task digest token savings and workspace-L0 latency)
- *(kb)* regenerate repo knowledge base with OKF frontmatter cards (stage 2 gate)
- *(audit)* add v0.4 context-audit metrics (bsl compression, scan and staleness latency)
- *(audit)* add v0.3 context-audit metrics (MCP cold start, router fallback, token budget compliance)
- *(release)* add multi-OS CI workflow, cargo-dist config, and release-plz pipeline (D26, D27)
- *(build)* add mise.toml tasks, binstall/crates.io metadata, drop lefthook sync-binary (D26, D27)
- dogfood repo storage for context-init
- repo storage mode
- *(plan)* repo storage implementation plan
- *(spec)* repo storage design
- *(agents)* sorted navigation map with file-module wording
- *(agents)* refresh navigation map after per-module subsystem split
- *(reports)* step 5 final review report
- *(reports)* step 5 summary, handoff status, refreshed AGENTS.md map
- *(cli)* [**breaking**] thin binary over ci-* crates; add --model/--role/--config
- *(ci-core)* generator, deterministic subsystems and AST packer in analysis
- *(ci-core)* move project detector into ci_core::analysis
- *(agents)* initialize OpenViking knowledge base and AGENTS.md navigation
- *(benchmark)* add context audit and performance benchmark reports
- expand README and add CLI cheatsheet
- add integration test suite for CLI and AST packing
- add lefthook with rust check, fmt and host binary sync

### Added
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
