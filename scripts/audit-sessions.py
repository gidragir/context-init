#!/usr/bin/env python3
"""Stdlib-only transcript auditor for Claude Code and Antigravity sessions."""

import argparse
import glob
import json
import os
import re
from collections import Counter
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Tuple

COUNTER_NAMES = ("ci_mcp", "ci_cli", "read", "search_shell", "bash", "files")

CI_CLI_RE = re.compile(
    r"\bcontext-init\s+(?:init|update|check|pack|search|task|scan|workspace)\b"
)
SEARCH_SHELL_RE = re.compile(r"(?:^|[\s;|&()`])(?:grep|rg|find|fd)(?=$|[\s;|&()`])")
ANTIGRAVITY_TOOL_RE = re.compile(rb"context-init[_:]+context_[a-z_]+")


def new_counters() -> Dict[str, int]:
    return {name: 0 for name in COUNTER_NAMES}


def read_subagent_type(jsonl_path: Path) -> str:
    meta_path = jsonl_path.with_suffix(".meta.json")
    try:
        raw = meta_path.read_text(encoding="utf-8", errors="replace")
        meta = json.loads(raw)
        if isinstance(meta, dict):
            agent_type = meta.get("agentType")
            if isinstance(agent_type, str) and agent_type:
                return f"subagent:{agent_type}"
    except OSError:
        pass
    except ValueError:
        pass
    return "subagent:unknown"


def iter_tool_uses(line: str) -> Iterable[Tuple[str, dict]]:
    try:
        obj = json.loads(line)
    except ValueError:
        return
    if not isinstance(obj, dict):
        return
    msg = obj.get("message")
    if isinstance(msg, dict):
        content = msg.get("content")
    else:
        content = obj.get("content")
    if not isinstance(content, list):
        return
    for block in content:
        if not isinstance(block, dict):
            continue
        if block.get("type") != "tool_use":
            continue
        name = block.get("name")
        if not isinstance(name, str):
            continue
        inp = block.get("input")
        if not isinstance(inp, dict):
            inp = {}
        yield name, inp


def audit_transcript_file(jsonl_path: Path, counts: Dict[str, int]) -> None:
    counts["files"] += 1
    try:
        with jsonl_path.open("r", encoding="utf-8", errors="replace") as fh:
            for line in fh:
                line = line.strip()
                if not line:
                    continue
                for name, inp in iter_tool_uses(line):
                    if name.startswith("mcp__context-init__"):
                        counts["ci_mcp"] += 1
                    elif name == "Read":
                        counts["read"] += 1
                    elif name == "Bash":
                        counts["bash"] += 1
                        cmd = inp.get("command")
                        if isinstance(cmd, str):
                            if CI_CLI_RE.search(cmd):
                                counts["ci_cli"] += 1
                            if SEARCH_SHELL_RE.search(cmd):
                                counts["search_shell"] += 1
    except OSError:
        return


def audit_claude_dir(
    claude_dir: Path, project_substr: Optional[str]
) -> Dict[str, Dict[str, Dict[str, int]]]:
    results: Dict[str, Dict[str, Dict[str, int]]] = {}
    if not claude_dir.is_dir():
        return results

    try:
        proj_dirs = sorted(p for p in claude_dir.iterdir() if p.is_dir())
    except OSError:
        return results

    for proj_dir in proj_dirs:
        proj_name = proj_dir.name
        if project_substr and project_substr not in proj_name:
            continue

        agent_map: Dict[str, Dict[str, int]] = {}

        # Main transcripts: <claude_dir>/<project>/<session>.jsonl
        try:
            main_files = sorted(
                p for p in proj_dir.iterdir() if p.is_file() and p.suffix == ".jsonl"
            )
        except OSError:
            main_files = []

        for jf in main_files:
            counts = agent_map.setdefault("main", new_counters())
            audit_transcript_file(jf, counts)

        # Subagent transcripts: <claude_dir>/<project>/<session>/subagents/*.jsonl
        try:
            session_dirs = sorted(p for p in proj_dir.iterdir() if p.is_dir())
        except OSError:
            session_dirs = []

        for sess_dir in session_dirs:
            sub_dir = sess_dir / "subagents"
            if not sub_dir.is_dir():
                continue
            try:
                sub_files = sorted(
                    p for p in sub_dir.iterdir() if p.is_file() and p.suffix == ".jsonl"
                )
            except OSError:
                continue
            for sjf in sub_files:
                agent_type = read_subagent_type(sjf)
                counts = agent_map.setdefault(agent_type, new_counters())
                audit_transcript_file(sjf, counts)

        if agent_map:
            results[proj_name] = agent_map

    return results


def resolve_antigravity_dirs(arg: Optional[str]) -> List[Path]:
    pattern = os.path.expanduser(arg) if arg else os.path.expanduser(
        "~/.gemini/antigravity*/conversations"
    )
    matched = sorted(glob.glob(pattern))
    if matched:
        return [Path(p) for p in matched if Path(p).is_dir()]
    direct = Path(pattern)
    if direct.is_dir():
        return [direct]
    return []


def audit_antigravity(dirs: Iterable[Path]) -> Tuple[int, Counter]:
    db_count = 0
    tool_counts: Counter = Counter()
    for d in dirs:
        if not d.is_dir():
            continue
        try:
            db_files = sorted(d.rglob("*.db"))
        except OSError:
            continue
        for db_file in db_files:
            if not db_file.is_file():
                continue
            try:
                data = db_file.read_bytes()
            except OSError:
                continue
            db_count += 1
            for match in ANTIGRAVITY_TOOL_RE.findall(data):
                tool_counts[match.decode("ascii", errors="replace")] += 1
    return db_count, tool_counts


def format_json_output(
    claude_results: Dict[str, Dict[str, Dict[str, int]]],
    ag_dbs: int,
    ag_tools: Counter,
) -> str:
    out: Dict[str, object] = {}
    for proj, agents in claude_results.items():
        proj_out: Dict[str, Dict[str, int]] = {}
        for agent_type, counts in agents.items():
            nonzero = {k: counts[k] for k in COUNTER_NAMES if counts.get(k, 0) != 0}
            if nonzero:
                proj_out[agent_type] = nonzero
        if proj_out:
            out[proj] = proj_out
    out["antigravity"] = {"dbs": ag_dbs, "tools": dict(ag_tools)}
    return json.dumps(out, indent=2, sort_keys=True)


def format_table_output(
    claude_results: Dict[str, Dict[str, Dict[str, int]]],
    ag_dbs: int,
    ag_tools: Counter,
) -> str:
    headers = ["project", "agent", *COUNTER_NAMES]
    rows: List[List[str]] = []
    for proj in sorted(claude_results.keys()):
        agents = claude_results[proj]
        for agent_type in sorted(agents.keys()):
            counts = agents[agent_type]
            rows.append(
                [proj, agent_type, *(str(counts[c]) for c in COUNTER_NAMES)]
            )

    widths = [len(h) for h in headers]
    for row in rows:
        for i, cell in enumerate(row):
            if len(cell) > widths[i]:
                widths[i] = len(cell)

    def fmt_row(cells: List[str]) -> str:
        parts = []
        for i, cell in enumerate(cells):
            if i < 2:
                parts.append(cell.ljust(widths[i]))
            else:
                parts.append(cell.rjust(widths[i]))
        return "  ".join(parts)

    lines = [fmt_row(headers), "  ".join("-" * w for w in widths)]
    for row in rows:
        lines.append(fmt_row(row))

    tools_summary = (
        ", ".join(f"{k}={v}" for k, v in sorted(ag_tools.items()))
        if ag_tools
        else "none"
    )
    lines.append(f"antigravity: dbs={ag_dbs} tools={tools_summary}")
    return "\n".join(lines)


def parse_args(argv: Optional[List[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Audit Claude Code and Antigravity transcripts for context-init adoption."
    )
    parser.add_argument(
        "--claude",
        default="~/.claude/projects",
        help="Claude projects directory (default: ~/.claude/projects)",
    )
    parser.add_argument(
        "--antigravity",
        default=None,
        help="Antigravity conversations directory or glob (default: ~/.gemini/antigravity*/conversations)",
    )
    parser.add_argument(
        "--project",
        default=None,
        help="Filter Claude project directory names by substring",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Emit JSON output instead of aligned table",
    )
    return parser.parse_args(argv)


def main(argv: Optional[List[str]] = None) -> int:
    args = parse_args(argv)
    claude_dir = Path(os.path.expanduser(args.claude))
    claude_results = audit_claude_dir(claude_dir, args.project)
    ag_dirs = resolve_antigravity_dirs(args.antigravity)
    ag_dbs, ag_tools = audit_antigravity(ag_dirs)

    if args.json:
        print(format_json_output(claude_results, ag_dbs, ag_tools))
    else:
        print(format_table_output(claude_results, ag_dbs, ag_tools))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
