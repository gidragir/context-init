---
directory: crates/ci-llm/src/schema.rs
source:
  kind: git
  blob_sha: b0db935e3df6e8f119a2e763614fbd51c4445ac7
  commit_sha: 36198169b6c3e1dac832cf4204e7b96c6805df94
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

# Подсистема: Ci Llm Schema (L1)

Модуль-файл `crates/ci-llm/src/schema.rs`

> **Идентификатор:** `crates_ci-llm_schema`
> **Теги:** Rust (Cargo), crates_ci-llm_schema

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-llm/src/schema.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub fn extract_json_payload(raw: &str) -> &str
pub fn parse_and_validate_json(raw: &str, schema: Option<&Value>, tokens_in: usize, ) -> Result<Value>
pub fn validate_json_schema(schema: &Value, instance: &Value) -> Result<()>
pub fn default_role_json_schema(role: RoleName) -> Value
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-llm
```
