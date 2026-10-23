# Step 3 Implementation Report — `crates/ci-llm` & `crates/ci-core` Audit Fixes (F1–F5)

## 1. Scope & Architectural Decisions Covered

- **D13–D17 (`crates/ci-llm`)**:
  - **R1 (`LlmProvider` Trait & Request/Response Models)**: Implemented `RoleName` (`RoleKind` alias for `init`, `update`, `task`, `search`), `LlmRequest`, `LlmResponse`, `ProviderCapabilities`, `AgentWorkPackage`, `ValidatedAgentOutput`, `LlmExecutionError`, and `trait LlmProvider: Send + Sync` in [`crates/ci-llm/src/provider.rs`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/provider.rs).
  - **R2 (Middleware / Decorators)**: Implemented `RetryDecorator`, `TimeoutDecorator`, `TokenBudgetDecorator`, and `TelemetryDecorator` in [`crates/ci-llm/src/decorators.rs`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/decorators.rs).
  - **R3 (Provider Implementations & Extension Points)**: Implemented `OllamaProvider` (`<endpoint>/api/generate`, `<endpoint>/api/tags`, `is_ollama_online`, `parse_subsystem_summaries`), `CommandProvider` (stdin piping, `{prompt}`/`{system_prompt}`/`{model}` placeholder substitution, exit code check, timeout process termination, `BrokenPipe` handling), `AgentProvider` + `validate_agent_submission` (strict JSON Schema validation via [`crates/ci-llm/src/schema.rs`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/schema.rs)), extension points (`OpenAiCompatProvider`, `AnthropicProvider`, `GeminiProvider`, `SamplingProvider`), `ProviderFactory`, and `ProviderRegistry` in [`crates/ci-llm/src/providers.rs`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/providers.rs).
  - **R4 (`RoleRouter` & Fallback Chains)**: Implemented `RoleRouter` and `RoleExecutionOutcome` (`Deterministic` vs `Completed`, sequential fallback chain traversal, `fallback_used = true` propagation to both `LlmResponse` and `LlmSignal`, retry/schema error accumulation across attempts, and `model_override`) in [`crates/ci-llm/src/router.rs`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/router.rs).
  - **R5 (Language-Neutral Prompt Templates)**: Implemented `PromptTemplateRegistry`, `PromptTemplateSource`, `PromptVars`, and `RenderedPrompt` in [`crates/ci-llm/src/prompts.rs`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/prompts.rs), parameterized by `Config.language` with override discovery in `<config_dir>/prompts/<role>.{md,txt}` and `<repo>/.context-init/prompts/<role>.{md,txt}`.
- **D29 (Telemetry Integration)**:
  - `TelemetryDecorator` and `RoleRouter` emit `ci_core::telemetry::TelemetrySignal::Llm(LlmSignal)` into `Arc<dyn TelemetrySink>` and maintain full 10-field `LlmTelemetryRecord` snapshots (`model_alias`, `provider_kind`, `role`, `tokens_in`, `tokens_out`, `latency_ms`, `fallback_used`, `schema_error`, `retries`, `success`).
- **R6 (`crates/ci-core` Audit Findings F1–F5)**:
  - Closed all 5 non-blocking findings from [`reports/subagents/ci-core_verification.md`](file:///home/gidragir/projects/context-init/reports/subagents/ci-core_verification.md) and added regression coverage in [`crates/ci-core/src/lib.rs`](file:///home/gidragir/projects/context-init/crates/ci-core/src/lib.rs).

---

## 2. Files Created & Modified

| File | Change Summary |
| :--- | :--- |
| [`crates/ci-llm/Cargo.toml`](file:///home/gidragir/projects/context-init/crates/ci-llm/Cargo.toml) | Added `schemars.workspace = true`. |
| [`crates/ci-llm/src/provider.rs`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/provider.rs) | Core LLM request/response models, `LlmProvider` trait, `AgentWorkPackage`, `ValidatedAgentOutput`, `LlmExecutionError`, and token estimation utilities. |
| [`crates/ci-llm/src/schema.rs`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/schema.rs) | Markdown code-fence stripper (`extract_json_payload` resilient to nested backticks inside JSON strings and braces in prose), recursive JSON Schema validator (`validate_json_schema` with local `#/...` `$ref` resolution, `parse_and_validate_json`), and per-role default JSON Schemas (`default_role_json_schema`). |
| [`crates/ci-llm/src/decorators.rs`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/decorators.rs) | `RetryDecorator` (skips non-retryable budget and extension point errors), `TimeoutDecorator`, `TokenBudgetDecorator`, and `TelemetryDecorator` (`LlmTelemetryRecord` + `LlmSignal` emission). |
| [`crates/ci-llm/src/providers.rs`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/providers.rs) | `OllamaProvider` (with error field & timeout classification), `CommandProvider` (single-pass placeholder substitution, non-blocking background stdin streaming for `>64 KB` payloads), `AgentProvider` + `validate_agent_submission` (rejects non-string `package_id`), extension points (`OpenAiCompatProvider`, `AnthropicProvider`, `GeminiProvider`, `SamplingProvider`), `ProviderFactory`, and `ProviderRegistry`. |
| [`crates/ci-llm/src/router.rs`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/router.rs) | `RoleRouter` and `RoleExecutionOutcome` with fallback chain traversal, `"none"` deterministic mode, `model_override`, and failure telemetry propagation even when the final alias in a chain is missing from `ProviderRegistry`. |
| [`crates/ci-llm/src/prompts.rs`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/prompts.rs) | Language-neutral `PromptTemplateRegistry` parameterized by `Config.language` with single-pass placeholder substitution and global/repo-level `.md`/`.txt` overrides. |
| [`crates/ci-llm/src/lib.rs`](file:///home/gidragir/projects/context-init/crates/ci-llm/src/lib.rs) | Public re-exports and 10 comprehensive unit tests covering R1–R5 and adversarial edge cases. |
| [`crates/ci-core/src/storage.rs`](file:///home/gidragir/projects/context-init/crates/ci-core/src/storage.rs) | **F1 fix**: `normalize_git_remote` only strips numeric `:port/` segments when `had_scheme` is true, preserving SCP-style `git@host:1234/repo.git` path segments. |
| [`crates/ci-core/src/config.rs`](file:///home/gidragir/projects/context-init/crates/ci-core/src/config.rs) | **F2 & F3 fixes**: `StoragePaths::resolve` and `Config::load` pass `None` to `resolve_with_env` so unset `HOME` is not masked by `dirs::home_dir()`; empty/whitespace `home_dir` is filtered out; `fallback_global_config_path` is only populated from `APPDATA` when `CONTEXT_INIT_CONFIG` and `XDG_CONFIG_HOME` are unset. |
| [`crates/ci-core/src/openviking.rs`](file:///home/gidragir/projects/context-init/crates/ci-core/src/openviking.rs) | **F4 fix**: `CardFrontmatter::to_yaml` escapes scalars containing `#`, `:`, `{`, `"`, `'`, `\`, or leading/trailing whitespace via `format_yaml_scalar`, with symmetric unescaping in `unquote_yaml_value`. |
| [`crates/ci-core/src/git.rs`](file:///home/gidragir/projects/context-init/crates/ci-core/src/git.rs) | **F5 fix**: `check_frontmatter_staleness` treats `status_porcelain_z` failure (`Err`) as `status_lookup_failed = true` and `stale = true` instead of `.unwrap_or_default()`. |
| [`crates/ci-core/src/lib.rs`](file:///home/gidragir/projects/context-init/crates/ci-core/src/lib.rs) | Added `regression_f1_to_f5_audit_fixes` unit test verifying F1–F5. |

---

## 3. Verification Summary

- `rtk cargo check -p ci-llm`: **PASS** (exit code 0)
- `rtk cargo check --workspace`: **PASS** (exit code 0)
- `rtk cargo clippy -p ci-llm --all-targets -- -D warnings`: **PASS** (0 warnings)
- `rtk cargo clippy --workspace --all-targets -- -D warnings`: **PASS** (0 warnings)
- `rtk cargo nextest run -p ci-llm`: **PASS** (10/10 tests passed)
- `rtk cargo nextest run --workspace`: **PASS** (63/63 tests passed across 6 binaries)
- `rtk rg -i '1[cс]' Cargo.toml crates/ src/`: **PASS** (0 matches)
- `context_check` (MCP `context-init`): **PASS** (`has_drift: false`, 26 subsystems up-to-date)
