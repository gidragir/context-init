---
directory: crates/ci-core/src/openviking.rs
source:
  kind: git
  blob_sha: d0748321868e0379c157555376f25c17ac24b0f7
  commit_sha: fff7dd106863757218c552cb8ef5b18e46c70899
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

# Подсистема: Формат OpenViking/OKF (L1)

Типы frontmatter OKF-карточек: CardSource, CardGeneratedBy, CardFreshness, CardFrontmatter; лимит L0-аннотации 256 символов.

> **Идентификатор:** `crates_ci-core_openviking`
> **Теги:** okf, openviking, schema

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/src/openviking.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub const L0_ABSTRACT_MAX_CHARS: usize = 256
pub struct CardSource
pub struct CardGeneratedBy
pub struct CardFreshness
pub struct CardFrontmatter
pub fn to_yaml(&self) -> String
pub fn parse_yaml(yaml_text: &str) -> Result<Self>
pub struct OkfCard
pub fn new(mut frontmatter: CardFrontmatter, body: impl Into<String>) -> Self
pub fn parse(markdown: &str) -> Result<Self>
pub fn to_markdown(&self) -> String
pub fn l0_abstract(&self) -> String
pub fn extract_l0_abstract(markdown: &str) -> String
pub fn extract_l0_abstract_with_limit(markdown: &str, max_chars: usize) -> String
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
