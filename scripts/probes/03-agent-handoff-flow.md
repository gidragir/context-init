# Probe 03: Agent Handoff Flow

## Question
Как устроен механизм передачи задач внешнему LLM-агенту (agent handoff): как формируется рабочий пакет (work package), где он регистрируется и как валидируется и применяется ответ агента?

## Kind
overview

## Expected
- `crates/ci-llm/src/provider.rs:15`: Структура `AgentWorkPackage` определяет контракт рабочего пакета для внешнего агента: `package_id`, `role`, `facts`, `instruction`, `json_schema`, `alias`, `deep`, `agents_md`, `project_root`.
- `crates/ci-llm/src/providers.rs:768`: Метод `AgentProvider::build_work_package` собирает пакет с уникальным ID вида `pkg-{alias}-{role}-{seq}` и целевой JSON-схемой при вызове LLM-провайдера с типом `ModelKind::Agent`.
- `crates/ci-mcp/src/server.rs:32`: MCP-сервер хранит активные рабочие пакеты в `work_packages: Arc<RwLock<HashMap<String, AgentWorkPackage>>>` и регистрирует их через `register_work_package` (`crates/ci-mcp/src/server.rs:120`).
- `crates/ci-mcp/src/handlers.rs:612`: Обработчик `context_submit` проверяет принадлежность пакета проекту (`project_root`), валидирует ответ агента по JSON-схеме через `validate_agent_submission` (`crates/ci-llm/src/providers.rs:851`) и применяет результат однократно (single-use).
