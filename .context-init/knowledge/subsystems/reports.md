---
directory: reports
source:
  kind: git
  blob_sha: efe363e70fea8405fa69e6d3b985ff3b77da5594
  commit_sha: e9daa96fc49222863d0ca3be1c21d8edee9a512b
generated_by:
  component: context-init
  role: init
  model: none
freshness:
  total_entries: 15
  sampled_entries: 15
  unsampled_entries: 0
  pending_child_changes: 0
---

# Подсистема: Reports (L1)

Модуль и компоненты директории `reports` (15 файлов)

> **Идентификатор:** `reports`
> **Теги:** Rust (Cargo), reports

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `reports/BENCHMARK_REPORT.md` | Исходный код |
| `reports/adoption_probe_2026-10.md` | Исходный код |
| `reports/benchmark_latest.json` | Исходный код |
| `reports/subagents/ci-core_summary.md` | Исходный код |
| `reports/subagents/ci-core_verification.md` | Исходный код |
| `reports/subagents/ci-llm_summary.md` | Исходный код |
| `reports/subagents/ci-llm_verification.md` | Исходный код |
| `reports/subagents/ci-mcp_summary.md` | Исходный код |
| `reports/subagents/ci-mcp_verification.md` | Исходный код |
| `reports/subagents/context-init_summary.md` | Исходный код |
| `reports/subagents/context-init_verification.md` | Исходный код |
| `reports/subagents/crate_split_summary.md` | Исходный код |
| `reports/subagents/step1_workspace_summary.md` | Исходный код |
| `reports/subagents/step1_workspace_verification.md` | Исходный код |
| `reports/subagents/v0.3_handoff.md` | Исходный код |

## 🧪 Запуск и тестирование
```bash
rtk mise run test
```
