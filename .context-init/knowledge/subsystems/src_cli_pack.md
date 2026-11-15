---
directory: src/cli/pack.rs
source:
  kind: git
  blob_sha: 497fc9bde36b4ddcaa9bb0ade8c213c42f7c701c
  commit_sha: b040800534c30a22a394125140a07068cb5d9d25
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
