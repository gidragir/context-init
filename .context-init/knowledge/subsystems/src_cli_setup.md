---
directory: src/cli/setup
source:
  kind: git
  blob_sha: fbe69a4f242517021a84621c17ed52ed253ef16d
  commit_sha: e9daa96fc49222863d0ca3be1c21d8edee9a512b
generated_by:
  component: context-init
  role: init
  model: none
freshness:
  total_entries: 5
  sampled_entries: 5
  unsampled_entries: 0
  pending_child_changes: 0
---

# Подсистема: Cli Setup (L1)

Подсистема `src/cli/setup` с ключевыми точками входа: src/cli/setup/mod.rs

> **Идентификатор:** `src_cli_setup`
> **Теги:** Rust (Cargo), src_cli_setup

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `src/cli/setup/harness.rs` | Исходный код |
| `src/cli/setup/merge.rs` | Исходный код |
| `src/cli/setup/mod.rs` | ⭐ Точка входа |
| `src/cli/setup/rule_template.md` | Исходный код |
| `src/cli/setup/rules.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub struct SetupArgs
pub enum SetupTarget
pub struct SetupPlan
pub fn plan_setup(h: Harness, dirs: &HomeDirs, exe: &Path) -> Result<SetupPlan>
pub fn plan_setup_with_rtk(h: Harness, dirs: &HomeDirs, exe: &Path, rtk: bool, ) -> Result<SetupPlan>
pub fn run(args: &SetupArgs) -> Result<ExitCode>
pub struct HomeDirs
pub enum Harness
pub enum ConfigFormat
pub enum RuleTarget
```

## 🧪 Запуск и тестирование
```bash
rtk mise run test
```
