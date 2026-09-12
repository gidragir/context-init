# 🚀 context-init

CLI-утилита на Rust для автоматической инициализации трехуровневой базы знаний (OpenViking L0-L2) в любых проектах.

## Установка и сборка

```bash
# Быстрая сборка:
cargo build --release

# Установка в систему:
cp target/release/context-init ~/.local/bin/
```

## Использование

```bash
# 1. Зайти в любой проект
cd ~/projects/my-service

# 2. Инициализировать контекст
context-init .

# 3. Или с интеллектуальным описанием подсистем через локальную Ollama (qwen2.5-coder:14b):
context-init --ai .
```

## Подробное руководство по коду и архитектуре Rust
Читайте в [docs/rust_context_init_tutorial.md](../../docs/rust_context_init_tutorial.md).
