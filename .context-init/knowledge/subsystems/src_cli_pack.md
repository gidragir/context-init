---
directory: src/cli/pack.rs
source:
  kind: git
  blob_sha: 60b1220bbd535adb5612567f7deb1ca0c9f2d1e4
  commit_sha: a37e2f209483ac97bb4729dec70449be893293c4
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

# Подсистема: Cli Pack (L1)

Модуль-файл `src/cli/pack.rs`

> **Идентификатор:** `src_cli_pack`
> **Теги:** Rust (Cargo), src_cli_pack

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `src/cli/pack.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub struct PackArgs
pub struct BudgetLevel
pub struct CliBudget<'a>
pub fn apply_cli_budget(levels: &[BudgetLevel], budget: &CliBudget<'_>, query: &[&str], render: &mut dyn FnMut(&BudgetLevel) -> Result<String>, ) -> Result<String>
pub fn run(global: &GlobalArgs, args: &PackArgs) -> Result<ExitCode>
```

## 🧪 Запуск и тестирование
```bash
rtk mise run test
```
