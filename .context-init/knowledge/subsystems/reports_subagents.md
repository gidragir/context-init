---
directory: reports/subagents
source:
  kind: git
  blob_sha: 54e17a4b0fc2655ac095ea0779def96e0ad5fd83
  commit_sha: fff7dd106863757218c552cb8ef5b18e46c70899
generated_by:
  component: context-init
  role: update
  model: none
freshness:
  total_entries: 7
  sampled_entries: 7
  unsampled_entries: 0
  pending_child_changes: 0
---

# Подсистема: Отчёты субагентов (L1)

Сводки работы субагентов по крейтам ci-core, ci-llm, ci-mcp, корню и разделению workspace.

> **Идентификатор:** `reports_subagents`
> **Теги:** reports, subagents

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `reports/subagents/ci-core_summary.md` | Исходный код |
| `reports/subagents/ci-llm_summary.md` | Исходный код |
| `reports/subagents/ci-mcp_summary.md` | Исходный код |
| `reports/subagents/context-init_summary.md` | Исходный код |
| `reports/subagents/crate_split_summary.md` | Исходный код |
| `reports/subagents/step1_workspace_summary.md` | Исходный код |
| `reports/subagents/v0.3_handoff.md` | Исходный код |

## 🧪 Запуск и тестирование
```bash
cargo test
```
