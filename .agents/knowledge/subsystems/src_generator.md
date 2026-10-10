# Подсистема: Generator (L1)

> **Идентификатор:** `src_generator`
> **Теги:** src_generator, Rust (Cargo) | **Объем:** ~5565 токенов BPE

## 🎯 Назначение и роль
Модуль и компоненты директории `src/generator` (1 файлов)

## 📁 Точки входа и структура файлов

| Файл | Строк | Статус |
| :--- | :--- | :--- |
| `src/generator.rs` | 554 | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub fn generate_knowledge_base(analysis: &ProjectAnalysis, subsystems: &[SubsystemInfo], force: bool, with_agents_md: bool, ) -> Result<()>
pub fn audit_knowledge_base(analysis: &ProjectAnalysis) -> Result<DriftReport>
pub fn update_knowledge_base(analysis: &ProjectAnalysis, new_subsystems: &[SubsystemInfo], with_agents_md: bool, ) -> Result<(usize, usize)>
pub fn sanitize_id(raw: &str) -> String
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test
```
