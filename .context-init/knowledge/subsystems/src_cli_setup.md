---
directory: src/cli/setup
source:
  kind: git
  blob_sha: fbe69a4f242517021a84621c17ed52ed253ef16d
  commit_sha: fff7dd106863757218c552cb8ef5b18e46c70899
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

# Подсистема: CLI setup (L1)

context-init setup: план регистрации MCP-сервера и короткого правила агента для харнессов (Claude Code, Antigravity и др.), слияние с существующими конфигами.

> **Идентификатор:** `src_cli_setup`
> **Теги:** cli, harness, mcp, setup

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
```

## 🧪 Запуск и тестирование
```bash
cargo test
```
