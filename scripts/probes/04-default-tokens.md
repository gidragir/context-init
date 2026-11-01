# Probe 04: Default Tokens

## Question
Каковы точные значения по умолчанию для лимита токенов ответа MCP-сервера и для порога разбиения модулей на подсистемы, и где эти константы объявлены в кодовой базе?

## Kind
pointed

## Expected
- `crates/ci-core/src/config.rs:17`: Константа `DEFAULT_MAX_RESPONSE_TOKENS: usize = 4000;` задает максимальный бюджет токенов на один ответ MCP-инструмента.
- `crates/ci-core/src/config.rs:20`: Константа `DEFAULT_SPLIT_THRESHOLD_TOKENS: usize = 15_000;` задает порог объема исходного кода модуля в токенах BPE, выше которого модуль разбивается на подсистемы.
- `crates/ci-core/src/config.rs:23`: Константа `MIN_RESPONSE_TOKENS: usize = 64;` определяет минимальный допустимый лимит ответа, необходимый для размещения футера пагинации.
- `crates/ci-core/src/config.rs:688`: Поля `max_response_tokens` и `split_threshold_tokens` структуры `Config` используют значения этих констант по умолчанию через атрибуты serde.
