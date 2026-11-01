# Probe 01: Telemetry Wiring

## Question
Как устроена система сбора операционной телеметрии в проекте: какие сигналы собираются, как обеспечивается приватность и исключение путей к файлам или содержимого кода, и где инициализируются приемники (sink) для CLI и MCP-сервера?

## Kind
overview

## Expected
- `crates/ci-core/src/telemetry.rs:23`: Структура `OperationalSignal` собирает строго обезличенные метаданные вызова (`tool`, `channel`, `harness`, `latency_ms`, `success`, `error_class`, `version`, `os`, `nested`) без путей к файлам, исходного кода или содержимого запросов.
- `crates/ci-core/src/telemetry.rs:362`: Трейт `TelemetrySink` и его реализация `JsonlTelemetrySink` (`crates/ci-core/src/telemetry.rs:392`), сохраняющая события в локальный JSONL-файл в директории данных хранилища при opt-in (`telemetry.enabled`).
- `crates/ci-mcp/src/server.rs:742`: MCP-сервер в `run_mcp_server` создает `JsonlTelemetrySink` на базе `StoragePaths::resolve(default_root).data_dir` и подключает его через `ContextInitServer::with_telemetry_sink`.
- `src/cli/mod.rs:186`: CLI-команды инициализируют `telemetry_scope` с `Channel::Cli` и логируют завершение вызовов через `record_call`, только если `project.config.telemetry.enabled` включен.
