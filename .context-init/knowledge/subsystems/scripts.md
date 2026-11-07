---
directory: scripts
source:
  kind: git
  blob_sha: 2676993145e9f10831c150e5e35ccae06cd42acd
  commit_sha: 05b17436dd0087469fb8bef6c61f16e554831faa
generated_by:
  component: context-init
  role: init
  model: none
freshness:
  total_entries: 9
  sampled_entries: 9
  unsampled_entries: 0
  pending_child_changes: 0
---

# Подсистема: Scripts (L1)

Модуль и компоненты директории `scripts` (9 файлов)

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
| `scripts/tests/fixtures/claude/proj/main/subagents/agent-1.meta.json` | Исходный код |
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
```

## 🧪 Запуск и тестирование
```bash
rtk mise run test
```
