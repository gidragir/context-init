---
directory: crates/ci-core/src/analysis/task.rs
source:
  kind: git
  blob_sha: fdb882030d4c2b20f999f64f8670ea2f7e09576f
  commit_sha: 9660354a570d6203128acd3df79cd62f23660f76
generated_by:
  component: context-init
  role: update
  model: none
freshness:
  total_entries: 1
  sampled_entries: 1
  unsampled_entries: 0
  pending_child_changes: 0
---

# Подсистема: Ci Core Analysis Task (L1)

Модуль-файл `crates/ci-core/src/analysis/task.rs`

> **Идентификатор:** `crates_ci-core_analysis_task`
> **Теги:** Rust (Cargo), crates_ci-core_analysis_task

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/src/analysis/task.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub struct TaskDigestRequest<'a>
pub fn build_task_digest(root: &Path, git: &dyn GitBackend, index: Option<&L0Index>, req: &TaskDigestRequest, ) -> Result<String>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
