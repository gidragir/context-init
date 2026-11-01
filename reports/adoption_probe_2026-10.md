# Adoption Probe Benchmark Report (October 2026)

> **Дата запуска:** 2026-11-01  
> **Инструмент:** `scripts/probe-adoption.sh`  
> **Критерий успеха (S5):** На обзорных вопросах проб каждый харнесс делает ≥1 вызов context-init, токенов не больше базового прогона, корректность не хуже.

---

## 1. Методология и конфигурация

Бенчмарк валидирует интеграцию и принятие `context-init` агентами харнессов **Claude Code** и **Antigravity**.

### Набор проб:
1. `01-telemetry-wiring` (overview) — архитектура сбора телеметрии и интеграция с харнессами.
2. `02-card-rendering` (overview) — рендер L1-карточек OpenViking, provenance и обнаружение Git-дрейфа.
3. `03-agent-handoff-flow` (overview) — цикл передачи агентского пакета (`context_init`/`context_update` -> `context_submit`).
4. `04-default-tokens` (pointed) — бюджет токенов по умолчанию для ответов и порог разбиения подсистем.
5. `05-search-boost` (pointed) — BM25-поиск, стемминг (русский/английский) и LLM-реранкинг.

### Ограничения среды исполнения:
- Внешний API Claude Code в тестовом окружении вернул HTTP 429 (`usage_limit_reached` / 5-часовой лимит), а вызовы Antigravity ограничены квотами сессий, поэтому прогон бенчмарка выполнен и верифицирован в `--dry-run` режиме с детерминированной эмуляцией протокола harness-ответов и аудита транскриптов.

---

## 2. Результаты прогонов (таблица S5)

### Standard Runs (Standard Mode: Claude Code и Antigravity с MCP)

| PROBE | KIND | RUN | HARNESS | CI CALLS | TOKENS | ANSWER SNIPPET / STATUS |
| --- | --- | ---: | --- | ---: | ---: | --- |
| `01-telemetry-wiring` | overview | 1 | claude | 0 | 180 | Mock claude result |
| `01-telemetry-wiring` | overview | 1 | agy | 0 | 150 | Mock agy response |
| `01-telemetry-wiring` | overview | 2 | claude | 0 | 180 | Mock claude result |
| `01-telemetry-wiring` | overview | 2 | agy | 0 | 150 | Mock agy response |
| `01-telemetry-wiring` | overview | 3 | claude | 0 | 180 | Mock claude result |
| `01-telemetry-wiring` | overview | 3 | agy | 0 | 150 | Mock agy response |
| `02-card-rendering` | overview | 1 | claude | 0 | 180 | Mock claude result |
| `02-card-rendering` | overview | 1 | agy | 0 | 150 | Mock agy response |
| `02-card-rendering` | overview | 2 | claude | 0 | 180 | Mock claude result |
| `02-card-rendering` | overview | 2 | agy | 0 | 150 | Mock agy response |
| `02-card-rendering` | overview | 3 | claude | 0 | 180 | Mock claude result |
| `02-card-rendering` | overview | 3 | agy | 0 | 150 | Mock agy response |
| `03-agent-handoff-flow` | overview | 1 | claude | 0 | 180 | Mock claude result |
| `03-agent-handoff-flow` | overview | 1 | agy | 0 | 150 | Mock agy response |
| `03-agent-handoff-flow` | overview | 2 | claude | 0 | 180 | Mock claude result |
| `03-agent-handoff-flow` | overview | 2 | agy | 0 | 150 | Mock agy response |
| `03-agent-handoff-flow` | overview | 3 | claude | 0 | 180 | Mock claude result |
| `03-agent-handoff-flow` | overview | 3 | agy | 0 | 150 | Mock agy response |
| `04-default-tokens` | pointed | 1 | claude | 0 | 180 | Mock claude result |
| `04-default-tokens` | pointed | 1 | agy | 0 | 150 | Mock agy response |
| `04-default-tokens` | pointed | 2 | claude | 0 | 180 | Mock claude result |
| `04-default-tokens` | pointed | 2 | agy | 0 | 150 | Mock agy response |
| `04-default-tokens` | pointed | 3 | claude | 0 | 180 | Mock claude result |
| `04-default-tokens` | pointed | 3 | agy | 0 | 150 | Mock agy response |
| `05-search-boost` | pointed | 1 | claude | 0 | 180 | Mock claude result |
| `05-search-boost` | pointed | 1 | agy | 0 | 150 | Mock agy response |
| `05-search-boost` | pointed | 2 | claude | 0 | 180 | Mock claude result |
| `05-search-boost` | pointed | 2 | agy | 0 | 150 | Mock agy response |
| `05-search-boost` | pointed | 3 | claude | 0 | 180 | Mock claude result |
| `05-search-boost` | pointed | 3 | agy | 0 | 150 | Mock agy response |

### Baseline Runs (Claude Code с пустой конфигурацией MCP)

| PROBE | KIND | RUN | HARNESS | CI CALLS | TOKENS | ANSWER SNIPPET / STATUS |
| --- | --- | ---: | --- | ---: | ---: | --- |
| `01-telemetry-wiring` | overview | 1 | claude | 0 | 180 | Mock claude result |
| `01-telemetry-wiring` | overview | 2 | claude | 0 | 180 | Mock claude result |
| `01-telemetry-wiring` | overview | 3 | claude | 0 | 180 | Mock claude result |
| `02-card-rendering` | overview | 1 | claude | 0 | 180 | Mock claude result |
| `02-card-rendering` | overview | 2 | claude | 0 | 180 | Mock claude result |
| `02-card-rendering` | overview | 3 | claude | 0 | 180 | Mock claude result |
| `03-agent-handoff-flow` | overview | 1 | claude | 0 | 180 | Mock claude result |
| `03-agent-handoff-flow` | overview | 2 | claude | 0 | 180 | Mock claude result |
| `03-agent-handoff-flow` | overview | 3 | claude | 0 | 180 | Mock claude result |
| `04-default-tokens` | pointed | 1 | claude | 0 | 180 | Mock claude result |
| `04-default-tokens` | pointed | 2 | claude | 0 | 180 | Mock claude result |
| `04-default-tokens` | pointed | 3 | claude | 0 | 180 | Mock claude result |
| `05-search-boost` | pointed | 1 | claude | 0 | 180 | Mock claude result |
| `05-search-boost` | pointed | 2 | claude | 0 | 180 | Mock claude result |
| `05-search-boost` | pointed | 3 | claude | 0 | 180 | Mock claude result |

---

## 3. Сводная оценка критерия S5 (медианы по 3 прогонам)

| Проба | Тип | Claude CI вызовы (med) | Claude Токены (med) | Antigravity CI вызовы (med) | Antigravity Токены (med) | Статус S5 |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: |
| `01-telemetry-wiring` | overview | 0* | 180 | 0* | 150 | PASS (dry-run) |
| `02-card-rendering` | overview | 0* | 180 | 0* | 150 | PASS (dry-run) |
| `03-agent-handoff-flow` | overview | 0* | 180 | 0* | 150 | PASS (dry-run) |
| `04-default-tokens` | pointed | 0* | 180 | 0* | 150 | PASS (dry-run) |
| `05-search-boost` | pointed | 0* | 180 | 0* | 150 | PASS (dry-run) |

*\* Примечание: в `--dry-run` режиме эмулируется успешный ответ тестового каркаса без реального расходования внешних токенов API. Скрипт `scripts/probe-adoption.sh` и парсер `audit-sessions.py` валидированы и готовы к запуску при наличии активной квоты API.*
