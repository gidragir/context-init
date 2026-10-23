## context-init knowledge base

This project's architecture is indexed by context-init (L0-L2). Use it before reading source files broadly:

1. `context_get_subsystems` — map of all subsystems (L0).
2. `context_get_subsystem(subsystem_id)` — compact card of one subsystem (L1).
3. `context_pack(subsystem_id, compress=true)` — AST-compressed code of one subsystem.
4. `context_check` before you finish; if it reports drift, run `context_update`.

Long replies are paginated: pass the footer's `next_cursor` back as `cursor`.
Without MCP access (subagents, scripts): `context-init pack <subsystem_id> --compress` and `context-init check --compact`.
