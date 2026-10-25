# Подсистема: Ci Llm Schema (L1)

> **Идентификатор:** `crates_ci-llm_schema`
> **Теги:** Rust (Cargo), crates_ci-llm_schema

## 🎯 Назначение и роль
Модуль-файл `crates/ci-llm/src/schema.rs`

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
