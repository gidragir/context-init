# Подсистема: Cli Setup (L1)

> **Идентификатор:** `src_cli_setup`
> **Теги:** Rust (Cargo), src_cli_setup

## 🎯 Назначение и роль
Подсистема `src/cli/setup` с ключевыми точками входа: src/cli/setup/mod.rs

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
