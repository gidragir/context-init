---
directory: src/cli/pack.rs
source:
  kind: git
  blob_sha: b15ee7af6f96b5a3866b3584b4199faebc5de2a2
  commit_sha: 8edfd693b819f9b4b54f664fed4f61310c0300c2
generated_by:
  component: context-init
  role: init
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
