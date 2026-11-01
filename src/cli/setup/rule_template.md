## context-init knowledge base

This project's architecture is indexed by context-init (L0-L2). Use it before reading source files broadly:

### Main session (MCP)

1. `context_get_subsystems` — map of all subsystems (L0).
2. `context_get_subsystem(subsystem_id)` — compact card of one subsystem (L1).
3. `context_pack(subsystem_id, compress=true)` — AST-compressed code of one subsystem.
4. `context_check` before you finish; if it reports drift, run `context_update`.

Long replies are paginated: pass the footer's `next_cursor` back as `cursor`.

### Subagents and scripts (CLI)

Before reading code broadly, run `{rtk}context-init search "<question>" --compact`,
then `{rtk}context-init pack <subsystem_id> --compress` for the best hit.
For a known symbol or string, `rg` is faster — use it.
