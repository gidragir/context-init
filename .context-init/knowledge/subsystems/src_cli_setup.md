---
directory: src/cli/setup
source:
  kind: git
  blob_sha: da88a67208e5fb87dda23da0776d816d90d3ac17
  commit_sha: 4f153c6ee310b681808626eb7348ac3ca484e21d
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
pub fn run(args: &SetupArgs) -> Result<ExitCode>
pub struct HomeDirs
pub fn from_env() -> Result<Self>
pub enum Harness
pub fn name(self) -> &'static str
pub enum ConfigFormat
pub enum RuleTarget
pub struct HarnessSpec
pub fn spec(h: Harness, dirs: &HomeDirs) -> HarnessSpec
pub fn is_installed(h: Harness, dirs: &HomeDirs) -> bool
pub fn server_entry(h: Harness, exe: &Path) -> Value
pub const SERVER_NAME: &str = "context-init"
pub fn merge_json(existing: Option<&str>, entry: Value) -> Result<String>
pub fn merge_toml(existing: Option<&str>, exe: &Path) -> Result<String>
pub const DIFF_MAX_LINES: usize = 200
pub const DIFF_MAX_LINE_CHARS: usize = 200
pub struct PlannedWrite
pub fn new(path: PathBuf, new: String) -> Result<Self>
pub fn is_noop(&self) -> bool
pub fn unified_diff(&self) -> String
pub fn apply(&self, stamp: &str) -> Result<Option<PathBuf>>
pub fn atomic_write_str(path: &Path, content: &str) -> Result<()>
pub const RULE_TEMPLATE: &str = include_str !("rule_template.md")
pub const BEGIN_MARKER: &str = "<!-- context-init:begin -->"
pub const END_MARKER: &str = "<!-- context-init:end -->"
pub fn upsert_marked_block(existing: Option<&str>, body: &str) -> String
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test
```
