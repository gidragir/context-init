# Changelog

## [Unreleased] — v0.3 «Фундамент»

### Breaking
- The binary is now a thin CLI over the `ci-core`, `ci-llm` and `ci-mcp` crates. `context-init [PATH]` with top-level flags is gone; use `context-init init [PATH]`. A bare `context-init` still runs `init .`.
- Removed `--ai`, `--ollama-url` and `-m/--model <ollama model>` from `init`/`update`. Configure models in `[models.<alias>]`, then select them with the global `--model <alias>` or `--role <role>=<alias>`. Deterministic mode (no LLM) is the default.
- `--with-agents-md` is replaced by `--no-agents-md` (the AGENTS.md navigation section is on by default).
- Default storage is the repository: `init` writes the knowledge base to `<repo>/.context-init/knowledge/` (`storage = "repo"`), to be committed with the code. `storage = "central"` keeps it in `~/.local/share/context-init/projects/<id>/`.
- `L0_index.json` schema v3: `root_path`, per-file `lines` and per-subsystem `tokens` are no longer stored (sizes are computed on demand); older bases must be rebuilt with `context-init init --force`.
- Support for the legacy `.agents/knowledge` layout is removed.
- `migrate` now takes `--to repo|central` and copies the knowledge base between the two storages.
- MCP `context_init`/`context_update` take `model` instead of `ai`. `context_check` reports subsystem ids in `missing_in_fs`.

### Added
- `update` writes only files whose bytes change and keeps a stable order, so re-running it on an unchanged tree leaves the working tree clean.
- MCP `context_init` performs real project detection (it was a placeholder) and can hand an agent work package to the harness (`kind = "agent"`).
- MCP server reads the client's `roots/list` (D21).
- Global `--config <FILE>`.

### Security
- `context_submit` validates subsystem ids, confines writes to the knowledge directory, and rejects unknown work packages.
