---
directory: reports
source:
  kind: git
  blob_sha: a2f8a1e2454b9e5454bbdf23314c9c81f9cb4b0b
  commit_sha: 04e787fc09f8a1918eaa133e0f27e78243d42c89
generated_by:
  component: context-init
  role: update
  model: none
freshness:
  total_entries: 2
  sampled_entries: 2
  unsampled_entries: 0
  pending_child_changes: 0
---

# Подсистема: Отчёты бенчмарков (L1)

BENCHMARK_REPORT.md и benchmark_latest.json — результаты регрессионного бенчмарка (токены, точность).

> **Идентификатор:** `reports`
> **Теги:** benchmark, reports

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `reports/BENCHMARK_REPORT.md` | Исходный код |
| `reports/benchmark_latest.json` | Исходный код |

## 🧪 Запуск и тестирование
```bash
cargo test
```
