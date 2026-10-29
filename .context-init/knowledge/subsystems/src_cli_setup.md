---
directory: src/cli/setup
source:
  kind: git
  blob_sha: da88a67208e5fb87dda23da0776d816d90d3ac17
  commit_sha: 6ef401530ecaf2a5ec6c1476e8b34c2cdb8a29d7
generated_by:
  component: context-init
  role: update
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
pub fn run(args: &SetupArgs) -> Result<ExitCode>
pub struct HomeDirs
pub enum Harness
pub enum ConfigFormat
pub enum RuleTarget
pub struct HarnessSpec
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test
```
