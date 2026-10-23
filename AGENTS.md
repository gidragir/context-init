# Инструкции для ИИ-агентов в проекте context-init

Данный проект настроен для автономной работы ассистентов.

## 🗺 Архитектурная карта и навигация (OpenViking L0-L2)

Проект `context-init` управляется трехуровневой базой знаний OpenViking (L0-L2).
Категорически запрещено читать файлы `.agents/knowledge/**` напрямую через инструменты чтения файлов (`view_file`, `cat` и др.). Вся навигация и исследование архитектуры выполняются исключительно через MCP-сервер `context-init` (MCP-First Protocol).

### ⚡ MCP-First Protocol (context-init)

1. **Обзор всех подсистем и структуры проекта (L0):**
   Вызовите `context_get_subsystems`:
   ```json
   {"ServerName": "context-init", "ToolName": "context_get_subsystems", "Arguments": {}}
   ```
2. **Получение компактной L1-карточки конкретной подсистемы:**
   Вызовите `context_get_subsystem` (передайте `subsystem_id`):
   ```json
   {"ServerName": "context-init", "ToolName": "context_get_subsystem", "Arguments": {"subsystem_id": "<subsystem_id>"}}
   ```
3. **AST-сжатый контекст кода подсистемы (без раздувания контекста):**
   Вызовите `context_pack`:
   ```json
   {"ServerName": "context-init", "ToolName": "context_pack", "Arguments": {"subsystem_id": "<subsystem_id>", "compress": true}}
   ```
4. **Проверка архитектурного дрейфа перед сдачей задачи:**
   Вызовите `context_check`:
   ```json
   {"ServerName": "context-init", "ToolName": "context_check", "Arguments": {}}
   ```

### 🖥 CLI-First Protocol (для субагентов и окружений без MCP)

Автономные субагенты-воркеры (`DeepCoderWorker`, `research`), не имеющие доступа к `call_mcp_tool`, обязаны использовать CLI вместо чтения файлов целиком через `view_file`:
1. **AST-сжатый контекст подсистемы:** `rtk context-init pack <subsystem_id> --compress` (или `rtk cargo run -- pack <subsystem_id> --compress`).
2. **Проверка дрейфа и валидация:** `rtk context-init check` (или `rtk cargo run -- check`).

### 📦 Доступные подсистемы:
- `src_core` (Main) — Подсистема `src/core` с ключевыми точками входа: src/main.rs
- `src_detector` (Detector) — Модуль и компоненты директории `src/detector` (1 файлов)
- `src_generator` (Generator) — Модуль и компоненты директории `src/generator` (1 файлов)
- `src_ollama` (Ollama) — Модуль и компоненты директории `src/ollama` (1 файлов)
- `src_types` (Types) — Модуль и компоненты директории `src/types` (1 файлов)
- `src_mcp` (Mcp) — Модуль и компоненты директории `src/mcp` (1 файлов)
- `src_service` (Service) — Модуль и компоненты директории `src/service` (1 файлов)
- `reports` (Reports) — Модуль и компоненты директории `reports` (2 файлов)
- `tests` (Tests) — Модуль и компоненты директории `tests` (1 файлов)
