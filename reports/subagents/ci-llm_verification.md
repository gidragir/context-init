# Отчет о независимой верификации Шага 3 вехи v0.3 «Фундамент» (Крейт `crates/ci-llm` и аудит-фиксы F1–F5 в `crates/ci-core`)

**Дата аудита:** 2026-10-20  
**Аудитор:** DeepInvestigator (Independent Adversarial Review & Verification)  
**Итоговый статус:** **PASS (все 9 пунктов задания и 18 критериев верификации выполнены в полном объеме, дефектов не обнаружено)**  
**Артефакт проверки:** [`reports/subagents/ci-llm_verification.md`](file:///home/gidragir/projects/context-init/reports/subagents/ci-llm_verification.md)

---

## 1. Сводная таблица верификации чек-листа

| # | Пункт чек-листа | Нормативный документ | Статус | Подтвержденное доказательство (файл, строки, тесты) |
| :--- | :--- | :--- | :--- | :--- |
| **1.1** | `trait LlmProvider: Send + Sync` (`complete`, `capabilities`, `kind`, `alias`) и blanket-реализации для `Box<dyn LlmProvider>`, `Arc<dyn LlmProvider>` | [milestone_decisions.md](file:///home/gidragir/projects/context-init/milestone_decisions.md#L58) (D13) | **PASS** | [`LlmProvider`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/provider.rs#L151-L163), blanket impls [`provider.rs:165-199`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/provider.rs#L165-L199), тест [`smoke_version_and_core_link`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/lib.rs#L214-L219) |
| **1.2** | Типы данных LLM: `LlmRequest`, `LlmResponse`, `ProviderCapabilities`, `RoleName`, `AgentWorkPackage`, `ValidatedAgentOutput`, `LlmExecutionError` | D13, D14, D16 | **PASS** | [`provider.rs:10-235`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/provider.rs#L10-L235), тесты [`agent_provider_and_validate_agent_submission_json_schema`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/lib.rs#L304-L384), [`decorators_retry_timeout_token_budget_and_telemetry`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/lib.rs#L221-L301) |
| **2.1** | Декораторы middleware: `RetryDecorator`, `TimeoutDecorator`, `TokenBudgetDecorator`, `TelemetryDecorator` | [milestone_decisions.md](file:///home/gidragir/projects/context-init/milestone_decisions.md#L58) (D13), [D29](file:///home/gidragir/projects/context-init/milestone_decisions.md#L94) | **PASS** | [`decorators.rs:1-526`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/decorators.rs#L1-L526), тест [`decorators_retry_timeout_token_budget_and_telemetry`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/lib.rs#L221-L301) |
| **2.2** | Фабрика `ProviderFactory` и реестр `ProviderRegistry` с автоматическим сбором декораторов из `Config` | D13, D16 | **PASS** | [`ProviderFactory`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/providers.rs#L1232-L1282), [`ProviderRegistry`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/providers.rs#L1285-L1342) |
| **3.1** | `OllamaProvider`: отсутствие захардкоженных URL, моделей и языковых директив; `parse_subsystem_summaries`, `is_ollama_online` | [milestone_decisions.md](file:///home/gidragir/projects/context-init/milestone_decisions.md#L59) (D14) | **PASS** | [`OllamaProvider`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/providers.rs#L136-L410), [`parse_subsystem_summaries`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/providers.rs#L58-L80), тест [`ollama_provider_parsing_and_mock_http_server`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/lib.rs#L448-L592) |
| **3.2** | `CommandProvider`: передача через stdin, однопроходная подстановка `{prompt}`, `{system_prompt}`, `{model}`, таймауты без дедлока пайпов | [milestone_decisions.md](file:///home/gidragir/projects/context-init/milestone_decisions.md#L59) (D14) | **PASS** | [`CommandProvider`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/providers.rs#L416-L741), тесты [`command_provider_stdin_placeholder_and_error_handling`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/lib.rs#L387-L445) и [`regression_nested_fences_large_stdin_timeout_single_pass_vars_and_schemars_ref`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/lib.rs#L1139-L1200) |
| **3.3** | `AgentProvider` и `validate_agent_submission`: строгая валидация JSON Schema, поддержка прямого объекта и конвертов (`result`, `output`, `submission`, `payload`) | [milestone_decisions.md](file:///home/gidragir/projects/context-init/milestone_decisions.md#L59) (D14) | **PASS** | [`AgentProvider`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/providers.rs#L747-L843), [`validate_agent_submission`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/providers.rs#L851-L981), тест [`agent_provider_and_validate_agent_submission_json_schema`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/lib.rs#L304-L384) |
| **3.4** | Точки расширения v0.3 без реализации: `OpenAiCompatProvider`, `AnthropicProvider`, `GeminiProvider`, `SamplingProvider` (структурированная `ExtensionPointError`) | [milestone_decisions.md](file:///home/gidragir/projects/context-init/milestone_decisions.md#L60) (D15) | **PASS** | [`providers.rs:984-1229`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/providers.rs#L984-L1229), тест [`extension_point_providers_return_structured_v03_error_and_capabilities`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/lib.rs#L595-L638) |
| **4.1** | `RoleRouter`: цепочки fallback для `init`, `update`, `task`, `search`, поддержка `"none"` (детерминированный режим), переопределение `model_override` | [milestone_decisions.md](file:///home/gidragir/projects/context-init/milestone_decisions.md#L61) (D16) | **PASS** | [`RoleRouter`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/router.rs#L71-L365), тест [`role_router_fallback_chain_none_mode_override_and_telemetry`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/lib.rs#L641-L757) |
| **4.2** | Телеметрия LLM (D29): фиксация `fallback_used = true`, накопление `retries` и `schema_errors` по всей цепочке fallback | [milestone_decisions.md](file:///home/gidragir/projects/context-init/milestone_decisions.md#L94) (D29) | **PASS** | [`RoleRouter::execute`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/router.rs#L164-L354), тест [`json_schema_validator_and_router_schema_error_edge_cases`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/lib.rs#L926-L981) |
| **5.1** | `PromptTemplateRegistry`: отсутствие захардкоженного `STRICTLY RUSSIAN` при смене языка, переопределение шаблонов из каталогов конфигурации | [milestone_decisions.md](file:///home/gidragir/projects/context-init/milestone_decisions.md#L62) (D17) | **PASS** | [`prompts.rs:1-544`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/prompts.rs#L1-L544), тест [`prompt_template_registry_language_parameterization_and_custom_overrides`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/lib.rs#L760-L865) |
| **6.1** | Закрытие замечания F1 в `crates/ci-core`: сохранение числового namespace в SCP-ссылках Git | [ci-core_verification.md](file:///home/gidragir/projects/context-init/reports/subagents/ci-core_verification.md#L133) (F1) | **PASS** | [`normalize_git_remote`](file:///home/gidragir/projects/context-init/crates/ci-core/src/storage.rs#L123-L138), тест [`regression_f1_to_f5_audit_fixes`](file:///home/gidragir/projects/context-init/crates/ci-core/src/lib.rs#L1103-L1118) |
| **6.2** | Закрытие замечания F2 в `crates/ci-core`: отсутствие затенения `HOME` / `USERPROFILE` в `StoragePaths::resolve` и `Config::load` | [ci-core_verification.md](file:///home/gidragir/projects/context-init/reports/subagents/ci-core_verification.md#L142) (F2) | **PASS** | [`StoragePaths::resolve`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L46-L48), [`Config::load`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L722-L724), тест [`regression_f1_to_f5_audit_fixes`](file:///home/gidragir/projects/context-init/crates/ci-core/src/lib.rs#L1119-L1137) |
| **6.3** | Закрытие замечания F3 в `crates/ci-core`: изоляция `%APPDATA%` fallback при явном `CONTEXT_INIT_CONFIG` или `XDG_CONFIG_HOME` | [ci-core_verification.md](file:///home/gidragir/projects/context-init/reports/subagents/ci-core_verification.md#L146) (F3) | **PASS** | [`StoragePaths::resolve_with_env`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L92-L111), тест [`regression_f1_to_f5_audit_fixes`](file:///home/gidragir/projects/context-init/crates/ci-core/src/lib.rs#L1138-L1181) |
| **6.4** | Закрытие замечания F4 в `crates/ci-core`: экранирование скаляров в `CardFrontmatter::to_yaml` при наличии `#`, `:`, кавычек и краевых пробелов | [ci-core_verification.md](file:///home/gidragir/projects/context-init/reports/subagents/ci-core_verification.md#L149) (F4) | **PASS** | [`format_yaml_scalar`](file:///home/gidragir/projects/context-init/crates/ci-core/src/openviking.rs#L451-L465), [`unquote_yaml_value`](file:///home/gidragir/projects/context-init/crates/ci-core/src/openviking.rs#L497-L519), тест [`regression_f1_to_f5_audit_fixes`](file:///home/gidragir/projects/context-init/crates/ci-core/src/lib.rs#L1183-L1208) |
| **6.5** | Закрытие замечания F5 в `crates/ci-core`: маркировка карточки как `stale = true` при сбое `status_porcelain_z` | [ci-core_verification.md](file:///home/gidragir/projects/context-init/reports/subagents/ci-core_verification.md#L152) (F5) | **PASS** | [`check_frontmatter_staleness`](file:///home/gidragir/projects/context-init/crates/ci-core/src/git.rs#L399-L411), тест [`regression_f1_to_f5_audit_fixes`](file:///home/gidragir/projects/context-init/crates/ci-core/src/lib.rs#L1210-L1258) |
| **7.1** | Отсутствие упоминаний «1С» / «1C» в `Cargo.toml`, `crates/`, `src/` | [milestone_decisions.md](file:///home/gidragir/projects/context-init/milestone_decisions.md#L117) (разд. 3) | **PASS** | `rtk rg -i '1[cс]' Cargo.toml crates/ src/` → exit code `1` (0 совпадений) |
| **8.1** | `rtk cargo check -p ci-llm` и `rtk cargo check --workspace` | Объективная проверка | **PASS** | Код возврата `0` (`0.04s` / `0.04s`) |
| **8.2** | `rtk cargo clippy -p ci-llm --all-targets -- -D warnings` и по всему `--workspace` | Объективная проверка (D26) | **PASS** | Код возврата `0` (`cargo clippy: No issues found`) |
| **8.3** | `rtk cargo nextest run -p ci-llm` и по всему `--workspace` | Объективная проверка (D26) | **PASS** | Код возврата `0` (`10 passed` в `ci-llm`, `63 passed` по всему `--workspace`) |

---

## 2. Глубокий критический анализ реализации (Adversarial Verification)

### 2.1. Топология декораторов и предотвращение дублирования телеметрии
- В фабрике `ProviderFactory::create_with_decorators` ([providers.rs:1255-1281](file:///home/gidragir/projects/context-init/crates/ci-llm/src/providers.rs#L1255-L1281)) декораторы компонуются снаружи внутрь в следующем порядке вызова:
  1. `TelemetryDecorator` (внешний контур): фиксирует полную латентность, число ретраев, ошибки схемы, признак fallback и статус завершения.
  2. `TokenBudgetDecorator`: производит проверку `tokens_in` до запуска провайдера. При превышении бюджета возвращает ошибку `budget_exceeded = true`, предотвращая холостые обращения к сети или подпроцессам.
  3. `RetryDecorator`: перехватывает транзиентные ошибки, пропуская без повторных попыток фатальные ошибки (`budget_exceeded` и `ExtensionPointError`).
  4. `TimeoutDecorator`: контролирует время отклика каждой отдельной попытки, предотвращая зависание единичного запроса.
  5. `raw` provider: базовый провайдер (`OllamaProvider`, `CommandProvider`, `AgentProvider`).
- **Изоляция телеметрии роутера:** Метод `ProviderRegistry::from_config` собирает провайдеры без провайдерного `TelemetryDecorator` ([providers.rs:1250-1252](file:///home/gidragir/projects/context-init/crates/ci-llm/src/providers.rs#L1250-L1252), [1302](file:///home/gidragir/projects/context-init/crates/ci-llm/src/providers.rs#L1302)). Телеметрия собирается и отправляется централизованно на уровне `RoleRouter::execute` ([router.rs:181-204, 271-300, 325-341](file:///home/gidragir/projects/context-init/crates/ci-llm/src/router.rs#L181-L204)), что исключает двойную фиксацию одного и того же вызова в телеметрическом логе при переходах по цепочке fallback.

### 2.2. Защита от дедлока пайпов и корректное завершение в `CommandProvider`
- В `CommandProvider::complete` ([providers.rs:552-580](file:///home/gidragir/projects/context-init/crates/ci-llm/src/providers.rs#L552-L580)) потоки `stdout` и `stderr` вычитываются в фоновых тредах сразу после вызова `child.spawn()`.
- Поток `stdin` также пишется в отдельном фоновом потоке. При срабатывании таймаута вызывается `child.kill()`, отправляющий `SIGKILL`. Закрытие дескриптора stdin в ядре ОС немедленно завершает запись в фоновом потоке ошибкой `EPIPE`, предотвращая зависание вызова `h.join()` ([providers.rs:593-601](file:///home/gidragir/projects/context-init/crates/ci-llm/src/providers.rs#L593-L601)).
- Однопроходный парсер `substitute_command_arg` ([providers.rs:702-741](file:///home/gidragir/projects/context-init/crates/ci-llm/src/providers.rs#L702-L741)) не производит повторного сканирования уже вставленного текста промпта, исключая случайную интерполяцию фигурных скобок `{...}`, содержащихся внутри пользовательского кода.

### 2.3. Изоляция JSON Schema и поддержка вложенного Markdown
- Функция `extract_json_payload` ([schema.rs:9-72](file:///home/gidragir/projects/context-init/crates/ci-llm/src/schema.rs#L9-L72)) использует стриминговый парсер `serde_json::Deserializer::from_str().into_iter::<IgnoredAny>()`, определяя точный байтовый оффсет валидного JSON.
- При наличии внешних markdown-блоков с тройными бэктиками поиск закрывающего маркера выполняется через `rfind("```")`, что исключает усечение JSON-объектов, содержащих внутренние тройные бэктики в строковых markdown-полях карточек.
- Валидатор `validate_json_schema` ([schema.rs:158-409](file:///home/gidragir/projects/context-init/crates/ci-llm/src/schema.rs#L158-L409)) корректно резолвит локальные ссылки `$ref` по спецификации RFC 6901 (`#/definitions/...` и `#/$defs/...`), поддерживая схемы, сгенерированные макросом `schemars::schema_for!`, и ограничивает глубину рекурсии лимитом `MAX_SCHEMA_REF_DEPTH = 64`.

### 2.4. Языковая нейтральность и каскад переопределений
- Шаблоны промптов параметризуются через `language_instruction(language)` ([prompts.rs:327-359](file:///home/gidragir/projects/context-init/crates/ci-llm/src/prompts.rs#L327-L359)). В шаблонах отсутствуют привязки к конкретным семействам моделей (отсутствуют упоминания `qwen` и директива `STRICTLY RUSSIAN`).
- Приоритет загрузки шаблонов:
  1. `<repo_root>/.context-init/prompts/<role>.{md,txt}` (`PromptTemplateSource::RepoConfig`)
  2. `<config_dir>/prompts/<role>.{md,txt}` (`PromptTemplateSource::GlobalConfig`)
  3. Встроенные шаблоны по умолчанию (`PromptTemplateSource::BuiltIn`)

### 2.5. Проверка устранения замечаний F1–F5 в `crates/ci-core`
- **F1 (`normalize_git_remote`):** В [crates/ci-core/src/storage.rs:123-134](file:///home/gidragir/projects/context-init/crates/ci-core/src/storage.rs#L123-L134) проверка отсечения порта дополнена `had_scheme`. Ссылки вида `git@github.com:12345/repo.git` сохраняют `12345` в пути: `github.com/12345/repo`.
- **F2 (`StoragePaths::resolve` / `Config::load`):** В [crates/ci-core/src/config.rs:46-48, 722-724](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L46-L48) аргумент `home_dir` передается как `None`, что открывает корректную проверку переменных `HOME` и `USERPROFILE` с фильтрацией пустых значений.
- **F3 (`fallback_global_config_path`):** В [crates/ci-core/src/config.rs:107-111](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs#L107-L111) при установке `CONTEXT_INIT_CONFIG` или `XDG_CONFIG_HOME` поле `fallback_global_config_path` принудительно обнуляется (`None`), предотвращая нежелательный откат к `%APPDATA%`.
- **F4 (`CardFrontmatter::to_yaml`):** В [crates/ci-core/src/openviking.rs:85-93, 451-465](file:///home/gidragir/projects/context-init/crates/ci-core/src/openviking.rs#L85-L93) строки с `#`, `:`, `{`, кавычками, обратными слешами и краевыми пробелами экранируются и квотируются через `format_yaml_scalar`.
- **F5 (`check_frontmatter_staleness`):** В [crates/ci-core/src/git.rs:399-411](file:///home/gidragir/projects/context-init/crates/ci-core/src/git.rs#L399-L411) сбой `backend.status_porcelain_z` устанавливает `status_lookup_failed = true`, переводя карточку в `stale = true` с диагностикой `"working tree status check failed"`.

---

## 3. Объективные метрики и проверки окружения

1. **Поиск запрещенных терминов («1С» / «1C»):**
   ```bash
   rtk rg -i '1[cс]' Cargo.toml crates/ src/
   # Exit code: 1 (0 совпадений)
   ```
2. **Компиляция workspace (`cargo check`):**
   ```text
   $ export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && rtk cargo check -p ci-llm && rtk cargo check --workspace
   Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.04s
   Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.04s
   Exit code: 0
   ```
3. **Статический анализ (`cargo clippy`):**
   ```text
   $ export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && rtk cargo clippy -p ci-llm --all-targets -- -D warnings && rtk cargo clippy --workspace --all-targets -- -D warnings
   cargo clippy: No issues found
   cargo clippy: No issues found
   Exit code: 0
   ```
4. **Тестовый прогон (`cargo nextest`):**
   ```text
   $ export PATH="${__MISE_ZSH_ACTIVATE_PATH:-$HOME/.local/share/mise/shims}:$PATH" && rtk cargo nextest run -p ci-llm && rtk cargo nextest run --workspace
   cargo nextest: 10 passed (1 binary, 0.072s)
   cargo nextest: 63 passed (6 binaries, 0.299s)
   Exit code: 0
   ```
5. **Проверка архитектурного дрейфа через MCP (`context_check`):**
   ```json
   {"has_drift":false,"message":"OK: 26 subsystems up-to-date","missing_in_fs":[],"untracked_in_knowledge":[],"up_to_date_count":26}
   ```

---

## 4. Remaining Questions & Gaps

1. **Интеграция интерфейсов `ci-llm` в MCP-сервер `ci-mcp` (Шаг 4):**
   Крейт `crates/ci-llm` полностью готов к интеграции. В рамках следующего шага потребуется:
   - Подключение `RoleRouter` для обслуживания инструментов `context_init`, `context_update`, `context_task`, `context_search`.
   - Подключение `AgentProvider::build_work_package` и `validate_agent_submission` к обработчику `context_submit`.
   - Инициализация `PromptTemplateRegistry` с учетом проектного и глобального каталогов шаблонов.
2. **Реализация MCP Sampling (`sampling/createMessage`) (D15):**
   Провайдер `SamplingProvider` подготовлен как расширение с флагом `is_agent_handoff: true`. Его реальное исполнение требует двунаправленного RPC-канала MCP-сессии с поддержкой `sampling` со стороны IDE/клиента (будет реализовано в `crates/ci-mcp`).
3. **Рекомендованный фокус для следующего этапа (Шаг 4):**
   Реализация крейта `crates/ci-mcp` согласно решениям D19–D22: жесткий потолок `max_response_tokens`, курсорная пагинация ответов, команда `setup` для внешних харнессов и интеграция с `ci-core` и `ci-llm`.
