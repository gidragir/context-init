## context-init knowledge base

This project's architecture is indexed by context-init (L0-L2). Use it before reading source files broadly.
Always pass `"path": "<repo_root>"`; the JSON below is ready to call without loading tool schemas.

### Main session (MCP)

1. `context_get_subsystems` `{"path":"<repo_root>"}` — map of all subsystems (L0).
2. `context_get_subsystem` `{"path":"<repo_root>","subsystem_id":"<id>"}` — compact card (L1).
3. `context_pack` `{"path":"<repo_root>","subsystem_id":"<id>","mode":"toc"}` — symbols with line spans.
4. `context_pack` `{"path":"<repo_root>","subsystem_id":"<id>","files":["<file>"],"compress":false}` — exact source.
   Default `style:"compact"` with `compress:true` is the densest AST-compressed view.
5. `context_check` `{"path":"<repo_root>"}` before you finish; on drift run `context_update`.

Reply ends with `truncated: true`? Repeat the call with `"cursor":"<next_cursor>"` or narrow `files`.

### Subagents and scripts (CLI)

Before reading code broadly, run `{rtk}context-init search "<question>" --compact`,
then `{rtk}context-init pack <subsystem_id> --compress --max-lines 200` for the best hit
(`--toc` for the outline, `--files <f> --no-compress` for bodies, `--cursor "cur_..."` for the next page).
For a known symbol or string, `rg` is faster — use it.
