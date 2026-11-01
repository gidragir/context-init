---
directory: crates/ci-llm/src/schema.rs
source:
  kind: git
  blob_sha: b95d2c22432aa435641bfb7304baeb4fc36bbdb0
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

# Подсистема: JSON-схемы ответов (L1)

Извлечение JSON из ответа модели, валидация по JSON Schema и схемы по умолчанию для каждой роли (default_role_json_schema).

> **Идентификатор:** `crates_ci-llm_schema`
> **Теги:** json-schema, llm, validation

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
