---
directory: reports
source:
  kind: git
  blob_sha: efe363e70fea8405fa69e6d3b985ff3b77da5594
  commit_sha: a19a2bff5a5f2cd044dc83a8f9c945a4281031de
generated_by:
  component: context-init
  role: update
  model: none
freshness:
  total_entries: 15
  sampled_entries: 15
  unsampled_entries: 0
  pending_child_changes: 0
---

# Подсистема: Reports (L1)

Модуль и компоненты директории `reports` (15 файлов)

> **Идентификатор:** `reports`
> **Теги:** Rust (Cargo), reports

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `reports/BENCHMARK_REPORT.md` | Исходный код |
| `reports/adoption_probe_2026-10.md` | Исходный код |
| `reports/benchmark_latest.json` | Исходный код |
| `reports/subagents/2026-11-15-context-init-improvements-research.md` | Исходный код |
| `reports/subagents/2026-11-15-context-init-improvements-review.md` | Исходный код |
| `reports/subagents/ci-core_verification.md` | Исходный код |
| `reports/subagents/ci-llm_summary.md` | Исходный код |
| `reports/subagents/ci-llm_verification.md` | Исходный код |
| `reports/subagents/ci-mcp_summary.md` | Исходный код |
| `reports/subagents/ci-mcp_verification.md` | Исходный код |
| `reports/subagents/context-init_verification.md` | Исходный код |
| `reports/subagents/crate_split_summary.md` | Исходный код |
| `reports/subagents/step1_workspace_summary.md` | Исходный код |
| `reports/subagents/step1_workspace_verification.md` | Исходный код |
| `reports/subagents/v0.3_handoff.md` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub enum PackStyle
pub enum PackMode
pub struct CompressOptions
pub strip_imports: bool, // Схлопывать import/use/package в `// [N imports omitted]` pub public_only: bool, // Оставлять только pub/export/public API pub exclude_tests: bool, // Вырезать #[cfg(...)] и test-модули/файлы (кроме ...
pub fn for_style(style: PackStyle) -> Self
pub trait LanguageAdapter: Send + Sync
pub fn pack_subsystem(...) -> Result<PackedSubsystems>
pub struct PackRequest<'a>
pub subsystem_id: &'a str, pub mode: PackMode, pub style: PackStyle, pub compress_opts: CompressOptions, /// Список относительных путей или glob-масок(...). pub files: &'a [String], /// Фильтр имён символов/методов дл...
pub files: Option<Vec<String>>, #[serde(...)] pub symbols: Option<Vec<String>>, #[serde(default)] pub mode: Option<String>, // "full" | "compressed" | "toc" ``` В `PackArgs` (`src/cli/pack.rs:18`): - `--files <GLOB_OR_PATH>` (repeatable / comma-separated) и алиас `--fil...
```

## 🧪 Запуск и тестирование
```bash
rtk mise run test
```
