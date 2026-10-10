# Подсистема: Ollama (L1)

> **Идентификатор:** `src_ollama`
> **Теги:** src_ollama, Rust (Cargo) | **Объем:** ~1183 токенов BPE

## 🎯 Назначение и роль
Модуль и компоненты директории `src/ollama` (1 файлов)

## 📁 Точки входа и структура файлов

| Файл | Строк | Статус |
| :--- | :--- | :--- |
| `src/ollama.rs` | 119 | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub struct SubsystemAiSummary
pub fn is_ollama_online(url: &str) -> bool
pub fn enrich_subsystems_with_ai(analysis: &ProjectAnalysis, ollama_url: &str, model: &str, ) -> Result<Vec<SubsystemAiSummary>>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test
```
