---
directory: crates/ci-core/src/analysis/lang/mdo.rs
source:
  kind: git
  blob_sha: 54e36e80fbf03e571080cd6f89ca243aa333de10
  commit_sha: ebecc37e77cdf411f5310fea0fda7166b9e27a50
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

# Подсистема: Ci Core Analysis Lang Mdo (L1)

Модуль-файл `crates/ci-core/src/analysis/lang/mdo.rs`

> **Идентификатор:** `crates_ci-core_analysis_lang_mdo`
> **Теги:** Rust (Cargo), crates_ci-core_analysis_lang_mdo

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/src/analysis/lang/mdo.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub struct MdoObject
pub fn parse_mdo_xml(xml: &str) -> Result<MdoObject>
pub fn semantic_diff(old: Option<&MdoObject>, new: &MdoObject) -> Vec<String>
pub fn render_mdo_l2_card(obj: &MdoObject) -> String
pub(crate) fn discover_mdo_subsystems(root: &Path, files: &[String], ) -> (Vec<DiscoveredModule>, BTreeSet<String>)
pub fn find_object_card(root: &Path, scope: &ScopeConfig, id: &str) -> Result<Option<String>>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
