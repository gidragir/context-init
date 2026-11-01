#!/usr/bin/env bash
#
# scripts/probe-adoption.sh
# Adoption probe benchmark for Claude Code and Antigravity harnesses.
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
PROBES_DIR="$REPO_ROOT/scripts/probes"

RUNS=1
BASELINE=false
PROBE_FILTER=""
DRY_RUN=false
TIMEOUT_SECS=300

usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Run adoption probes against Claude Code and Antigravity to benchmark
context-init usage, tokens, and answer accuracy.

Options:
    --runs N        Number of runs per probe (default: 1)
    --baseline      Baseline mode (run claude only with empty mcp config)
    --probe NAME    Run only probe matching NAME (e.g. 01, 01-telemetry-wiring)
    --dry-run       Dry-run mode (simulate harness calls without contacting LLM APIs)
    --timeout SECS  Per-turn timeout in seconds (default: 300)
    -h, --help      Show this help message
EOF
    exit 0
}

# Parse command line options
while [[ $# -gt 0 ]]; do
    case "$1" in
        --runs)
            RUNS="$2"
            shift 2
            ;;
        --runs=*)
            RUNS="${1#*=}"
            shift
            ;;
        --baseline)
            BASELINE=true
            shift
            ;;
        --probe)
            PROBE_FILTER="$2"
            shift 2
            ;;
        --probe=*)
            PROBE_FILTER="${1#*=}"
            shift
            ;;
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --timeout)
            TIMEOUT_SECS="$2"
            shift 2
            ;;
        --timeout=*)
            TIMEOUT_SECS="${1#*=}"
            shift
            ;;
        -h|--help)
            usage
            ;;
        *)
            echo "Error: unknown argument '$1'" >&2
            usage
            ;;
    esac
done

if [[ ! -d "$PROBES_DIR" ]]; then
    echo "Error: probes directory '$PROBES_DIR' does not exist" >&2
    exit 1
fi

# Discover probe markdown files
PROBE_FILES=()
for f in "$PROBES_DIR"/*.md; do
    if [[ -f "$f" ]]; then
        PROBE_FILES+=("$f")
    fi
done

if [[ ${#PROBE_FILES[@]} -eq 0 ]]; then
    echo "Error: no probe markdown files found in '$PROBES_DIR'" >&2
    exit 1
fi

# Apply optional filter
FILTERED_PROBES=()
for pf in "${PROBE_FILES[@]}"; do
    base="$(basename "$pf" .md)"
    if [[ -n "$PROBE_FILTER" ]]; then
        if [[ "$base" == *"$PROBE_FILTER"* || "$pf" == *"$PROBE_FILTER"* ]]; then
            FILTERED_PROBES+=("$pf")
        fi
    else
        FILTERED_PROBES+=("$pf")
    fi
done

if [[ ${#FILTERED_PROBES[@]} -eq 0 ]]; then
    echo "Error: no probes matched filter '$PROBE_FILTER'" >&2
    exit 1
fi

# Prepare output directory
TIMESTAMP="$(date +%Y%m%d_%H%M%S)"
OUTPUT_DIR="${TMPDIR:-/tmp}/context-init-probes/$TIMESTAMP"
mkdir -p "$OUTPUT_DIR"

echo "================================================================================"
echo "context-init adoption probe benchmark"
echo "Output directory: $OUTPUT_DIR"
echo "Probes: ${#FILTERED_PROBES[@]} | Runs per probe: $RUNS | Baseline: $BASELINE"
echo "================================================================================"

TMP_EMPTY_MCP=""
if [[ "$BASELINE" == "true" ]]; then
    TMP_EMPTY_MCP="$(mktemp -t empty_mcp_XXXXXX.json)"
    echo '{"mcpServers":{}}' > "$TMP_EMPTY_MCP"
fi

cleanup() {
    if [[ -n "$TMP_EMPTY_MCP" && -f "$TMP_EMPTY_MCP" ]]; then
        rm -f "$TMP_EMPTY_MCP"
    fi
}
trap cleanup EXIT

MANIFEST="$OUTPUT_DIR/runs.jsonl"
: > "$MANIFEST"

# Determine harnesses to run
if [[ "$BASELINE" == "true" ]]; then
    HARNESSES=("claude")
    echo "Mode: Baseline (Claude Code only with empty MCP config; Antigravity lacks override flag)"
else
    HARNESSES=("claude" "agy")
    echo "Mode: Standard (Claude Code and Antigravity with configured context-init MCP)"
fi

# Run probes
for pf in "${FILTERED_PROBES[@]}"; do
    probe_id="$(basename "$pf" .md)"

    # Extract Question and Kind via Python helper
    parsed_json="$(python3 -I - "$pf" <<'PYEOF'
import re, sys, json
from pathlib import Path

content = Path(sys.argv[1]).read_text(encoding="utf-8")
q_match = re.search(r"##\s+Question\s*\n(.*?)(?=\n##|\Z)", content, re.DOTALL)
k_match = re.search(r"##\s+Kind\s*\n(.*?)(?=\n##|\Z)", content, re.DOTALL)
e_match = re.search(r"##\s+Expected\s*\n(.*?)(?=\n##|\Z)", content, re.DOTALL)

q = q_match.group(1).strip() if q_match else ""
k = k_match.group(1).strip() if k_match else "unknown"
e = e_match.group(1).strip() if e_match else ""

print(json.dumps({"question": q, "kind": k, "expected": e}, ensure_ascii=False))
PYEOF
)"

    question="$(python3 -c "import json, sys; print(json.loads(sys.argv[1])['question'])" "$parsed_json")"
    kind="$(python3 -c "import json, sys; print(json.loads(sys.argv[1])['kind'])" "$parsed_json")"

    if [[ -z "$question" ]]; then
        echo "Warning: probe '$probe_id' has empty question, skipping" >&2
        continue
    fi

    echo ""
    echo "--- Probe: $probe_id (Kind: $kind) ---"

    for (( run=1; run<=RUNS; run++ )); do
        for harness in "${HARNESSES[@]}"; do
            echo -n "[Probe: $probe_id | Run: $run/$RUNS | Harness: $harness] Running... "

            stdout_file="$OUTPUT_DIR/${probe_id}_run${run}_${harness}.stdout"
            stderr_file="$OUTPUT_DIR/${probe_id}_run${run}_${harness}.stderr"
            exitcode_file="$OUTPUT_DIR/${probe_id}_run${run}_${harness}.exitcode"

            exit_code=0
            if [[ "$DRY_RUN" == "true" ]]; then
                if [[ "$harness" == "claude" ]]; then
                    echo '{"session_id":"mock-claude-session","usage":{"input_tokens":120,"output_tokens":60},"result":"Mock claude result","is_error":false}' > "$stdout_file"
                else
                    echo '{"event":"result","result":{"conversation_id":"mock-ag-conv","response":"Mock agy response","usage":{"input_tokens":110,"output_tokens":40,"total_tokens":150}}}' > "$stdout_file"
                fi
                : > "$stderr_file"
                echo "0" > "$exitcode_file"
                echo "DONE (dry-run)"
            else
                # Execute harness command in repository root
                set +e
                (
                    cd "$REPO_ROOT"
                    if [[ "$harness" == "claude" ]]; then
                        claude_prompt="${question} Делегируй исследование субагенту."
                        if [[ "$BASELINE" == "true" ]]; then
                            timeout "$TIMEOUT_SECS" claude --strict-mcp-config --mcp-config "$TMP_EMPTY_MCP" -p "$claude_prompt" --output-format json > "$stdout_file" 2> "$stderr_file"
                        else
                            timeout "$TIMEOUT_SECS" claude -p "$claude_prompt" --output-format json > "$stdout_file" 2> "$stderr_file"
                        fi
                    elif [[ "$harness" == "agy" ]]; then
                        timeout "$TIMEOUT_SECS" agy -p "$question" --mode plan --dangerously-skip-permissions --output-format stream-json > "$stdout_file" 2> "$stderr_file"
                    fi
                )
                exit_code=$?
                set -e
                echo "$exit_code" > "$exitcode_file"
                if [[ $exit_code -eq 0 ]]; then
                    echo "DONE (exit 0)"
                else
                    echo "FINISHED (exit $exit_code)"
                fi
            fi

            # Record in manifest
            python3 -I - "$MANIFEST" "$probe_id" "$kind" "$run" "$harness" "$stdout_file" "$stderr_file" "$exit_code" <<'PYEOF'
import json, sys
manifest_path, probe, kind, run, harness, stdout_f, stderr_f, code = sys.argv[1:9]
record = {
    "probe": probe,
    "kind": kind,
    "run": int(run),
    "harness": harness,
    "stdout": stdout_f,
    "stderr": stderr_f,
    "exit_code": int(code),
}
with open(manifest_path, "a", encoding="utf-8") as f:
    f.write(json.dumps(record, ensure_ascii=False) + "\n")
PYEOF
        done
    done
done

echo ""
echo "================================================================================"
echo "AUDITING SESSIONS AND GENERATING SUMMARY TABLE"
echo "================================================================================"

python3 -I - "$MANIFEST" "$REPO_ROOT" "$OUTPUT_DIR" <<'PYEOF'
import glob, json, os, subprocess, sys, tempfile
from pathlib import Path

manifest_path, repo_root, output_dir = sys.argv[1:4]
audit_script = os.path.join(repo_root, "scripts", "audit-sessions.py")

records = []
if os.path.isfile(manifest_path):
    with open(manifest_path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if line:
                records.append(json.loads(line))

results = []

for r in records:
    probe = r["probe"]
    kind = r["kind"]
    run = r["run"]
    harness = r["harness"]
    stdout_file = r["stdout"]
    exit_code = r["exit_code"]

    session_id = None
    tokens = 0
    snippet = ""
    ag_stream_ci_calls = 0
    text_deltas = []

    if os.path.isfile(stdout_file):
        raw = Path(stdout_file).read_text(encoding="utf-8", errors="replace")
        if harness == "claude":
            try:
                data = json.loads(raw)
                session_id = data.get("session_id")
                usage = data.get("usage") or {}
                tokens = usage.get("total_tokens")
                if tokens is None:
                    tokens = usage.get("input_tokens", 0) + usage.get("output_tokens", 0)
                snippet = data.get("result", "")
                if not snippet and data.get("is_error"):
                    snippet = f"API Error: {data.get('api_error', 'unknown')}"
            except Exception as e:
                snippet = f"JSON parse error: {e}"
        elif harness == "agy":
            try:
                for line in raw.splitlines():
                    line = line.strip()
                    if not line:
                        continue
                    try:
                        obj = json.loads(line)
                    except Exception:
                        continue
                    if obj.get("event") == "result":
                        res = obj.get("result", {})
                        session_id = res.get("conversation_id")
                        if res.get("response"):
                            snippet = res.get("response", "")
                        usage = res.get("usage", {})
                        tokens = usage.get("total_tokens")
                        if tokens is None:
                            tokens = usage.get("input_tokens", 0) + usage.get("output_tokens", 0)
                    elif obj.get("event") == "step_update":
                        su = obj.get("step_update", {})
                        if not session_id and "conversation_id" in su:
                            session_id = su["conversation_id"]
                        if su.get("text_delta"):
                            text_deltas.append(su["text_delta"])
                        # Count tool calls to context-init in agy stream
                        if su.get("state") == "ACTIVE" and su.get("step_type") == "tool":
                            t_info = su.get("tool_info", {})
                            params = t_info.get("parameters", {})
                            s_name = params.get("ServerName", "")
                            t_name = su.get("tool_name", "")
                            if s_name == "context-init" or t_name.startswith("mcp__context-init__"):
                                ag_stream_ci_calls += 1
                    elif not session_id:
                        if "conversation_id" in obj:
                            session_id = obj["conversation_id"]
            except Exception as e:
                snippet = f"Stream parse error: {e}"

    if not snippet and text_deltas:
        snippet = "".join(text_deltas)

    if not snippet:
        snippet = f"Exit code {exit_code}"

    # Normalize snippet: single line, at most 55 chars
    clean_snippet = " ".join(snippet.split())
    if len(clean_snippet) > 55:
        clean_snippet = clean_snippet[:52] + "..."

    # Audit context-init calls via scripts/audit-sessions.py --json
    ci_calls = 0
    if session_id:
        if harness == "claude":
            pattern = os.path.expanduser(f"~/.claude/projects/*/{session_id}.jsonl")
            matches = glob.glob(pattern)
            if matches:
                jf = Path(matches[0])
                with tempfile.TemporaryDirectory() as td:
                    proj_dir = Path(td) / "proj"
                    proj_dir.mkdir(parents=True, exist_ok=True)
                    try:
                        os.symlink(jf, proj_dir / jf.name)
                        sub_dir = jf.parent / session_id
                        if sub_dir.is_dir():
                            os.symlink(sub_dir, proj_dir / session_id)
                        res = subprocess.run(
                            [sys.executable, "-I", audit_script, "--claude", td, "--antigravity", "/dev/null", "--json"],
                            capture_output=True, text=True, timeout=30
                        )
                        if res.returncode == 0:
                            data = json.loads(res.stdout)
                            ci_calls = sum(
                                counts.get("ci_mcp", 0) + counts.get("ci_cli", 0)
                                for counts in data.get("proj", {}).values()
                            )
                    except Exception:
                        pass
        elif harness == "agy":
            pattern = os.path.expanduser(f"~/.gemini/antigravity*/conversations/{session_id}.db")
            matches = glob.glob(pattern)
            if matches:
                db = Path(matches[0])
                with tempfile.TemporaryDirectory() as td:
                    try:
                        os.symlink(db, Path(td) / db.name)
                        res = subprocess.run(
                            [sys.executable, "-I", audit_script, "--claude", "/dev/null", "--antigravity", td, "--json"],
                            capture_output=True, text=True, timeout=30
                        )
                        if res.returncode == 0:
                            data = json.loads(res.stdout)
                            ci_calls = sum(data.get("antigravity", {}).get("tools", {}).values())
                    except Exception:
                        pass
            if ag_stream_ci_calls > ci_calls:
                ci_calls = ag_stream_ci_calls

    results.append({
        "probe": probe,
        "kind": kind,
        "run": run,
        "harness": harness,
        "ci_calls": ci_calls,
        "tokens": tokens or 0,
        "snippet": clean_snippet,
        "session_id": session_id or "n/a",
        "output_file": stdout_file,
    })

# Format and print table
headers = ["PROBE", "KIND", "RUN", "HARNESS", "CI CALLS", "TOKENS", "ANSWER SNIPPET / STATUS"]
table_rows = []
for r in results:
    table_rows.append([
        r["probe"],
        r["kind"],
        str(r["run"]),
        r["harness"],
        str(r["ci_calls"]),
        str(r["tokens"]),
        r["snippet"],
    ])

widths = [len(h) for h in headers]
for row in table_rows:
    for i, cell in enumerate(row):
        widths[i] = max(widths[i], len(cell))

def fmt_row(cells):
    parts = []
    for i, cell in enumerate(cells):
        if i in (2, 4, 5):  # numeric columns right-aligned
            parts.append(cell.rjust(widths[i]))
        else:
            parts.append(cell.ljust(widths[i]))
    return "  ".join(parts)

print("")
print(fmt_row(headers))
print("  ".join("-" * w for w in widths))
for row in table_rows:
    print(fmt_row(row))
print("")

# Save summary markdown
summary_md = Path(output_dir) / "summary.md"
with open(summary_md, "w", encoding="utf-8") as f:
    f.write("# Adoption Probe Benchmark Summary\n\n")
    f.write(f"- Output directory: `{output_dir}`\n")
    f.write(f"- Total probe runs: {len(results)}\n\n")
    f.write("| " + " | ".join(headers) + " |\n")
    f.write("| " + " | ".join(["---"] * len(headers)) + " |\n")
    for r in results:
        cells = [
            r["probe"],
            r["kind"],
            str(r["run"]),
            r["harness"],
            str(r["ci_calls"]),
            str(r["tokens"]),
            r["snippet"].replace("|", "\\|"),
        ]
        f.write("| " + " | ".join(cells) + " |\n")
    f.write("\n## Raw Runs and Expected Anchors\n\n")
    for r in results:
        f.write(f"### `{r['probe']}` (run {r['run']}, {r['harness']})\n")
        f.write(f"- Kind: `{r['kind']}`\n")
        f.write(f"- Session ID: `{r['session_id']}`\n")
        f.write(f"- Context-init calls: `{r['ci_calls']}`\n")
        f.write(f"- Tokens: `{r['tokens']}`\n")
        f.write(f"- Output log: `{r['output_file']}`\n\n")

print(f"Summary written to: {summary_md}")
PYEOF
