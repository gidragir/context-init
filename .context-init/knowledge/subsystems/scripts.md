---
directory: scripts
source:
  kind: git
  blob_sha: 2676993145e9f10831c150e5e35ccae06cd42acd
  commit_sha: fff7dd106863757218c552cb8ef5b18e46c70899
generated_by:
  component: context-init
  role: update
  model: none
freshness:
  total_entries: 8
  sampled_entries: 8
  unsampled_entries: 0
  pending_child_changes: 0
---

# Подсистема: Scripts (L1)

Модуль и компоненты директории `scripts` (8 файлов)

> **Идентификатор:** `scripts`
> **Теги:** Rust (Cargo), scripts

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `scripts/audit-sessions.py` | Исходный код |
| `scripts/probe-adoption.sh` | Исходный код |
| `scripts/probes/01-telemetry-wiring.md` | Исходный код |
| `scripts/probes/02-card-rendering.md` | Исходный код |
| `scripts/probes/03-agent-handoff-flow.md` | Исходный код |
| `scripts/probes/04-default-tokens.md` | Исходный код |
| `scripts/probes/05-search-boost.md` | Исходный код |
| `scripts/tests/test_audit_sessions.py` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
def new_counters() -> Dict[str, int]
def read_subagent_type(jsonl_path: Path) -> str
def iter_tool_uses(line: str) -> Iterable[Tuple[str, dict]]
def audit_transcript_file(jsonl_path: Path, counts: Dict[str, int]) -> None
def audit_claude_dir( claude_dir: Path, project_substr: Optional[str] ) -> Dict[str, Dict[str, Dict[str, int]]]
def resolve_antigravity_dirs(arg: Optional[str]) -> List[Path]
def audit_antigravity(dirs: Iterable[Path]) -> Tuple[int, Counter]
def format_json_output( claude_results: Dict[str, Dict[str, Dict[str, int]]], ag_dbs: int, ag_tools: Counter, ) -> str
def format_table_output( claude_results: Dict[str, Dict[str, Dict[str, int]]], ag_dbs: int, ag_tools: Counter, ) -> str
def fmt_row(cells: List[str]) -> str
def parse_args(argv: Optional[List[str]] = None) -> argparse.Namespace
def main(argv: Optional[List[str]] = None) -> int
def fmt_row(...): parts = [] for i, cell in enumerate(cells): if i in (2, 4, 5): # numeric columns right-aligned parts.append(cell.rjust(widths[i])) else: parts.append(cell.ljust(widths[i])) return " ".join(parts) print("") print(fmt_row(headers)) print(" ".join("-" * w for w ...
```

## 🧪 Запуск и тестирование
```bash
cargo test
```
