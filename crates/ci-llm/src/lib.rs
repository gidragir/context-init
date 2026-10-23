//! LLM provider traits, decorators, MVP providers (`ollama`, `command`, `agent`),
//! extension points, role routing (`RoleRouter`), and prompt templates (`PromptTemplateRegistry`) for `context-init` (D13–D17, D29).

pub mod decorators;
pub mod enrich;
pub mod prompts;
pub mod provider;
pub mod providers;
pub mod router;
pub mod schema;

pub use decorators::{
    LlmTelemetryRecord, RetryDecorator, TelemetryDecorator, TimeoutDecorator, TokenBudgetDecorator,
};
pub use prompts::{
    language_instruction, PromptTemplate, PromptTemplateRegistry, PromptTemplateSource, PromptVars,
    RenderedPrompt,
};
pub use provider::{
    estimate_request_tokens, estimate_tokens, inspect_execution_error, AgentWorkPackage,
    LlmExecutionError, LlmProvider, LlmRequest, LlmResponse, ProviderCapabilities, RoleName,
    ValidatedAgentOutput,
};
pub use providers::{
    is_ollama_online, parse_subsystem_summaries, validate_agent_submission, AgentProvider,
    AnthropicProvider, CommandProvider, ExtensionPointError, GeminiProvider, OllamaProvider,
    OpenAiCompatProvider, ProviderFactory, ProviderRegistry, SamplingProvider, SubsystemAiSummary,
    DEFAULT_HEALTHCHECK_TIMEOUT_MS, DEFAULT_PROVIDER_TIMEOUT_MS,
};
pub use router::{RoleExecutionOutcome, RoleRouter};
pub use schema::{
    default_role_json_schema, extract_json_payload, parse_and_validate_json, validate_json_schema,
};

/// Current crate version synchronized with the workspace package metadata.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns the crate identifier and version string, linked with `ci-core`.
pub fn crate_info() -> String {
    format!("ci-llm/{} (core={})", VERSION, ci_core::VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::bail;
    use ci_core::config::{Config, ModelConfig, ModelKind, RoleKind};
    use ci_core::telemetry::{
        render_telemetry_report, JsonlTelemetrySink, LlmSignal, TelemetryEvent, TelemetrySignal,
        TelemetrySink,
    };
    use serde_json::json;
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(1);

    struct TempDirGuard {
        path: PathBuf,
    }

    impl TempDirGuard {
        fn new(prefix: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let seq = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let pid = std::process::id();
            let path = std::env::temp_dir().join(format!("ci_llm_{prefix}_{pid}_{nanos}_{seq}"));
            fs::create_dir_all(&path).expect("create temp test dir");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[derive(Default)]
    struct MemoryTelemetrySink {
        enabled: bool,
        events: Mutex<Vec<TelemetryEvent>>,
    }

    impl MemoryTelemetrySink {
        fn new(enabled: bool) -> Arc<Self> {
            Arc::new(Self {
                enabled,
                events: Mutex::new(Vec::new()),
            })
        }

        fn events(&self) -> Vec<TelemetryEvent> {
            self.events.lock().unwrap().clone()
        }

        fn llm_signals(&self) -> Vec<LlmSignal> {
            self.events()
                .into_iter()
                .filter_map(|e| match e.signal {
                    TelemetrySignal::Llm(s) => Some(s),
                    _ => None,
                })
                .collect()
        }
    }

    impl TelemetrySink for MemoryTelemetrySink {
        fn is_enabled(&self) -> bool {
            self.enabled
        }

        fn record(&self, event: &TelemetryEvent) -> anyhow::Result<()> {
            if self.enabled {
                self.events.lock().unwrap().push(event.clone());
            }
            Ok(())
        }
    }

    struct MockProvider {
        alias: String,
        kind: ModelKind,
        model: String,
        calls: Arc<AtomicU32>,
        fail_until_call: u32,
        sleep_ms: u64,
        reported_latency_ms: u64,
        response_content: String,
        max_context_tokens: Option<usize>,
    }

    impl MockProvider {
        fn succeeding(alias: &str, response_content: &str) -> Self {
            Self {
                alias: alias.to_string(),
                kind: ModelKind::Ollama,
                model: "mock-model".to_string(),
                calls: Arc::new(AtomicU32::new(0)),
                fail_until_call: 0,
                sleep_ms: 0,
                reported_latency_ms: 5,
                response_content: response_content.to_string(),
                max_context_tokens: None,
            }
        }

        fn failing(alias: &str, fail_count: u32, response_content: &str) -> Self {
            Self {
                fail_until_call: fail_count,
                ..Self::succeeding(alias, response_content)
            }
        }
    }

    impl LlmProvider for MockProvider {
        fn complete(&self, req: &LlmRequest) -> anyhow::Result<LlmResponse> {
            let call_no = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            if self.sleep_ms > 0 {
                thread::sleep(Duration::from_millis(self.sleep_ms));
            }
            if call_no <= self.fail_until_call {
                bail!("simulated failure on call {} for '{}'", call_no, self.alias);
            }
            Ok(LlmResponse {
                content: self.response_content.clone(),
                model: self.model.clone(),
                provider_kind: self.kind,
                alias: self.alias.clone(),
                tokens_in: estimate_request_tokens(req),
                tokens_out: estimate_tokens(&self.response_content),
                latency_ms: self.reported_latency_ms,
                retries: 0,
                fallback_used: false,
                work_package: None,
            })
        }

        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities {
                supports_json_schema: true,
                supports_streaming: false,
                is_agent_handoff: false,
                max_context_tokens: self.max_context_tokens,
            }
        }

        fn kind(&self) -> ModelKind {
            self.kind
        }

        fn alias(&self) -> &str {
            &self.alias
        }
    }

    #[test]
    fn smoke_version_and_core_link() {
        assert_eq!(VERSION, "0.3.0");
        assert_eq!(VERSION, ci_core::VERSION);
        assert!(crate_info().contains("ci-llm/0.3.0"));
    }

    #[test]
    fn decorators_retry_timeout_token_budget_and_telemetry() {
        // 1. RetryDecorator: succeeds after 2 transient failures (retries == 2)
        let mock_flaky = MockProvider::failing("flaky", 2, "{\"ok\": true}");
        let call_counter = Arc::clone(&mock_flaky.calls);
        let retry_prov = RetryDecorator::new(mock_flaky, 3);
        let req = LlmRequest::new(RoleName::Init, "Analyze repository modules");
        let resp = retry_prov.complete(&req).expect("succeed on 3rd attempt");
        assert_eq!(resp.retries, 2);
        assert_eq!(call_counter.load(Ordering::SeqCst), 3);

        // RetryDecorator exhaustion: fails when fail_until_call > max_retries and preserves retry count in metadata
        let mock_always_fails = MockProvider::failing("broken", 10, "never");
        let retry_exhausted = RetryDecorator::new(mock_always_fails, 2);
        let err = retry_exhausted.complete(&req).unwrap_err();
        let meta = inspect_execution_error(&err).expect("execution error metadata");
        assert_eq!(meta.retries, 2);

        // 2. TimeoutDecorator: rejects slow provider or 0ms budget
        let mut slow_mock = MockProvider::succeeding("slow", "done");
        slow_mock.sleep_ms = 80;
        let timeout_prov = TimeoutDecorator::with_timeout_ms(slow_mock, 15);
        let timeout_err = timeout_prov.complete(&req).unwrap_err();
        assert!(
            inspect_execution_error(&timeout_err).is_some_and(|m| m.timed_out),
            "Expected timeout error, got: {timeout_err}"
        );

        let fast_mock = MockProvider::succeeding("fast", "quick response");
        let zero_timeout_prov = TimeoutDecorator::new(fast_mock, None);
        let zero_req = LlmRequest::new(RoleName::Update, "hello").with_timeout_ms(0);
        assert!(zero_timeout_prov.complete(&zero_req).is_err());

        // 3. TokenBudgetDecorator: rejects input exceeding max_input_tokens BEFORE calling provider
        let guarded_mock = MockProvider::succeeding("budgeted", "short reply");
        let guarded_calls = Arc::clone(&guarded_mock.calls);
        let budget_prov = TokenBudgetDecorator::with_limit(guarded_mock, 5);
        let huge_req = LlmRequest::new(
            RoleName::Task,
            "one two three four five six seven eight nine ten eleven twelve",
        );
        let budget_err = budget_prov.complete(&huge_req).unwrap_err();
        assert!(inspect_execution_error(&budget_err).is_some_and(|m| m.budget_exceeded));
        assert_eq!(
            guarded_calls.load(Ordering::SeqCst),
            0,
            "Inner provider must NOT be called when input tokens exceed max_input_tokens"
        );

        // TokenBudgetDecorator allows input within budget and populates tokens_in / tokens_out
        let small_req = LlmRequest::new(RoleName::Task, "tiny");
        let ok_resp = budget_prov
            .complete(&small_req)
            .expect("within token budget");
        assert!(ok_resp.tokens_in > 0 && ok_resp.tokens_in <= 5);
        assert!(ok_resp.tokens_out > 0);
        assert_eq!(guarded_calls.load(Ordering::SeqCst), 1);

        // 4. TelemetryDecorator: records LlmSignal to TelemetrySink on both success and failure
        let sink = MemoryTelemetrySink::new(true);
        let inner_flaky = MockProvider::failing("tel_model", 1, "{\"status\": \"ok\"}");
        let with_retry = RetryDecorator::new(inner_flaky, 2);
        let tel_prov = TelemetryDecorator::new(with_retry, sink.clone() as Arc<dyn TelemetrySink>)
            .with_project_id("test-proj")
            .with_fallback_used(true);

        let tel_resp = tel_prov
            .complete(&small_req)
            .expect("succeed after 1 retry");
        assert!(tel_resp.fallback_used);
        assert_eq!(tel_resp.retries, 1);

        let signals = sink.llm_signals();
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].alias, "tel_model");
        assert_eq!(signals[0].kind, "ollama");
        assert_eq!(signals[0].role, "task");
        assert_eq!(signals[0].retries, 1);
        assert!(signals[0].fallback_used);
        assert_eq!(signals[0].schema_errors, 0);

        let last_rec = tel_prov.last_record().expect("telemetry record");
        assert!(last_rec.success);
        assert!(last_rec.fallback_used);
        assert_eq!(last_rec.retries, 1);
    }

    #[test]
    fn agent_provider_and_validate_agent_submission_json_schema() {
        let agent = AgentProvider::new("harness_agent");
        assert_eq!(agent.kind(), ModelKind::Agent);
        assert!(agent.capabilities().is_agent_handoff);
        assert!(agent.capabilities().supports_json_schema);

        let schema = default_role_json_schema(RoleName::Init);
        let req = LlmRequest::new(RoleName::Init, "Synthesize L1 cards")
            .with_system_prompt("Strict grounding.")
            .with_facts(json!({"modules": ["core", "llm"]}))
            .with_json_schema(schema);

        let resp = agent.complete(&req).expect("agent work package response");
        assert_eq!(resp.provider_kind, ModelKind::Agent);
        let pkg = resp.work_package.expect("AgentWorkPackage must be present");
        assert_eq!(pkg.role, RoleName::Init);
        assert!(pkg.package_id.starts_with("pkg-harness_agent-init-"));
        assert_eq!(pkg.facts, json!({"modules": ["core", "llm"]}));
        assert!(pkg.instruction.contains("Strict grounding."));

        // 1. Valid direct submission matching RoleName::Init schema
        let valid_direct = json!({
            "subsystems": [
                {
                    "id": "ci_core",
                    "name": "Core",
                    "summary": "Core configuration and storage primitives.",
                    "tags": ["rust", "core"]
                }
            ]
        });
        let validated = validate_agent_submission(&pkg, &valid_direct)
            .expect("valid direct submission must pass");
        assert_eq!(validated.package_id, pkg.package_id);
        assert_eq!(validated.role, RoleName::Init);
        assert_eq!(validated.payload, valid_direct);

        // 2. Valid envelope submission with matching package_id and result
        let valid_envelope = json!({
            "package_id": pkg.package_id,
            "result": valid_direct
        });
        let validated_env = validate_agent_submission(&pkg, &valid_envelope)
            .expect("valid envelope submission must pass");
        assert_eq!(validated_env.payload, valid_direct);

        // 3. Invalid envelope with wrong package_id must fail
        let wrong_pkg_id = json!({
            "package_id": "pkg-wrong-999",
            "result": valid_direct
        });
        assert!(validate_agent_submission(&pkg, &wrong_pkg_id).is_err());

        // 4. Invalid submission missing required field `summary` must fail
        let missing_field = json!({
            "subsystems": [
                {
                    "id": "ci_core",
                    "name": "Core",
                    "tags": ["rust"]
                }
            ]
        });
        assert!(validate_agent_submission(&pkg, &missing_field).is_err());

        // 5. Invalid submission with wrong type (`tags` is string instead of array) must fail
        let wrong_type = json!({
            "subsystems": [
                {
                    "id": "ci_core",
                    "name": "Core",
                    "summary": "Summary",
                    "tags": "not_an_array"
                }
            ]
        });
        assert!(validate_agent_submission(&pkg, &wrong_type).is_err());

        // 6. Null submission must fail
        assert!(validate_agent_submission(&pkg, &serde_json::Value::Null).is_err());
    }

    #[test]
    fn command_provider_stdin_placeholder_and_error_handling() {
        // Test cross-platform CLI execution using `git` (available in all test environments)
        // and platform shell for stdin/placeholder echo
        let git_cmd =
            CommandProvider::new("git_ver", vec!["git".to_string(), "--version".to_string()])
                .expect("valid CommandProvider");
        let req = LlmRequest::new(RoleName::Task, "ignored prompt");
        let resp = git_cmd.complete(&req).expect("git --version succeeds");
        assert_eq!(resp.provider_kind, ModelKind::Command);
        assert!(resp.content.contains("git version"));

        // Non-zero exit status must return Err
        let failing_git = CommandProvider::new(
            "git_fail",
            vec!["git".to_string(), "invalid-subcommand-xyz".to_string()],
        )
        .unwrap();
        assert!(failing_git.complete(&req).is_err());

        // Empty command must be rejected on construction
        assert!(CommandProvider::new("empty", vec![]).is_err());
        assert!(CommandProvider::new("blank", vec!["   ".to_string()]).is_err());

        // Test stdin piping and {prompt} placeholder substitution
        #[cfg(not(windows))]
        {
            // 1. Stdin piping via `cat`
            let cat_prov = CommandProvider::new("cat_stdin", vec!["cat".to_string()])
                .unwrap()
                .with_model("cli-cat");
            let stdin_req = LlmRequest::new(RoleName::Update, "payload_from_stdin")
                .with_system_prompt("system_header");
            let cat_resp = cat_prov.complete(&stdin_req).expect("cat reads stdin");
            assert_eq!(cat_resp.model, "cli-cat");
            assert!(cat_resp.content.contains("system_header"));
            assert!(cat_resp.content.contains("payload_from_stdin"));

            // 2. Placeholder `{prompt}` substitution via `sh -c 'printf "%s" "$1"' -- {prompt}`
            let placeholder_prov = CommandProvider::new(
                "echo_placeholder",
                vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "printf '%s' \"$1\"".to_string(),
                    "--".to_string(),
                    "{prompt}".to_string(),
                ],
            )
            .unwrap();
            let ph_req = LlmRequest::new(
                RoleName::Search,
                "{\"expanded_terms\": [\"ast\"], \"ranked_ids\": [\"core\"]}",
            )
            .with_json_schema(default_role_json_schema(RoleName::Search));
            let ph_resp = placeholder_prov
                .complete(&ph_req)
                .expect("placeholder substitution and schema validation");
            assert!(ph_resp.content.contains("expanded_terms"));
        }
    }

    #[test]
    fn ollama_provider_parsing_and_mock_http_server() {
        // 1. Test generalized `parse_subsystem_summaries` on fenced JSON, wrapper aliases, array, and single object
        let fenced = "```json\n{\"subsystems\": [{\"id\": \"core\", \"name\": \"Core\", \"summary\": \"Base module.\", \"tags\": [\"rust\"]}]}\n```";
        let s1 = parse_subsystem_summaries(fenced).expect("parse fenced wrapper");
        assert_eq!(s1.len(), 1);
        assert_eq!(s1[0].id, "core");

        let alias_modules = "{\"modules\": [{\"id\": \"llm\", \"name\": \"LLM\", \"summary\": \"Routing.\", \"tags\": []}]}";
        let s2 = parse_subsystem_summaries(alias_modules).expect("parse modules alias");
        assert_eq!(s2[0].id, "llm");

        let direct_arr = "[{\"id\": \"mcp\", \"name\": \"MCP\", \"summary\": \"Server.\", \"tags\": [\"stdio\"]}]";
        let s3 = parse_subsystem_summaries(direct_arr).expect("parse direct array");
        assert_eq!(s3[0].id, "mcp");

        let single_obj =
            "{\"id\": \"cli\", \"name\": \"CLI\", \"summary\": \"Binary.\", \"tags\": []}";
        let s4 = parse_subsystem_summaries(single_obj).expect("parse single object");
        assert_eq!(s4.len(), 1);
        assert_eq!(s4[0].id, "cli");

        assert!(parse_subsystem_summaries("").is_err());
        assert!(parse_subsystem_summaries("not valid json").is_err());

        // 2. Test OllamaProvider against a real local TCP HTTP mock server
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        let addr = listener.local_addr().unwrap();
        let endpoint = format!("http://{addr}");

        let server_handle = thread::spawn(move || {
            for _ in 0..4 {
                if let Ok((mut stream, _)) = listener.accept() {
                    let _ = stream.set_read_timeout(Some(Duration::from_millis(1000)));
                    let mut req_bytes = Vec::new();
                    let mut chunk = [0u8; 1024];
                    loop {
                        match stream.read(&mut chunk) {
                            Ok(0) => break,
                            Ok(n) => {
                                req_bytes.extend_from_slice(&chunk[..n]);
                                let cur = String::from_utf8_lossy(&req_bytes);
                                if let Some((hdrs, body_part)) = cur.split_once("\r\n\r\n") {
                                    let expected_len = hdrs
                                        .lines()
                                        .find_map(|l| {
                                            let lower = l.to_ascii_lowercase();
                                            lower
                                                .strip_prefix("content-length:")
                                                .and_then(|v| v.trim().parse::<usize>().ok())
                                        })
                                        .unwrap_or(0);
                                    if body_part.len() >= expected_len {
                                        break;
                                    }
                                }
                            }
                            Err(_) => break,
                        }
                    }
                    let req_str = String::from_utf8_lossy(&req_bytes);
                    let first_line = req_str.lines().next().unwrap_or("");

                    let (status, body) = if first_line.starts_with("GET /api/tags") {
                        (
                            "200 OK",
                            "{\"models\": [{\"name\": \"custom-local:7b\"}]}".to_string(),
                        )
                    } else if first_line.starts_with("POST /api/generate") {
                        if req_str.contains("TRIGGER_INVALID_SCHEMA") {
                            (
                                "200 OK",
                                serde_json::json!({
                                    "model": "custom-local:7b",
                                    "response": "{\"wrong_key\": 123}",
                                    "prompt_eval_count": 12,
                                    "eval_count": 5
                                })
                                .to_string(),
                            )
                        } else {
                            (
                                "200 OK",
                                serde_json::json!({
                                    "model": "custom-local:7b",
                                    "response": "```json\n{\"subsystems\": [{\"id\": \"core\", \"name\": \"Core\", \"summary\": \"Grounded summary.\", \"tags\": [\"rust\"]}]}\n```",
                                    "prompt_eval_count": 24,
                                    "eval_count": 18
                                })
                                .to_string(),
                            )
                        }
                    } else {
                        ("404 Not Found", "{}".to_string())
                    };

                    let http_resp = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(http_resp.as_bytes());
                    let _ = stream.flush();
                }
            }
        });

        let provider =
            OllamaProvider::new("local_ollama", &endpoint, "custom-local:7b").with_timeout_ms(5000);
        assert!(provider.is_online());
        let tags = provider.fetch_tags().expect("fetch tags");
        assert_eq!(tags, vec!["custom-local:7b".to_string()]);

        let valid_req = LlmRequest::new(RoleName::Init, "Generate subsystems")
            .with_json_schema(default_role_json_schema(RoleName::Init));
        let valid_resp = provider
            .complete(&valid_req)
            .expect("ollama generate 200 OK");
        assert_eq!(valid_resp.provider_kind, ModelKind::Ollama);
        assert_eq!(valid_resp.model, "custom-local:7b");
        assert_eq!(valid_resp.tokens_in, 24);
        assert_eq!(valid_resp.tokens_out, 18);

        let parsed_subs = parse_subsystem_summaries(&valid_resp.content).unwrap();
        assert_eq!(parsed_subs.len(), 1);
        assert_eq!(parsed_subs[0].id, "core");

        // Schema mismatch from Ollama must return a schema_error
        let bad_schema_req = LlmRequest::new(RoleName::Init, "TRIGGER_INVALID_SCHEMA")
            .with_json_schema(default_role_json_schema(RoleName::Init));
        let schema_err = provider.complete(&bad_schema_req).unwrap_err();
        assert!(inspect_execution_error(&schema_err).is_some_and(|m| m.schema_error));

        let _ = server_handle.join();

        // Missing endpoint or model must fail cleanly without network call
        let missing_ep = OllamaProvider::from_config(
            "no_ep",
            &ModelConfig {
                kind: ModelKind::Ollama,
                model: Some("m".to_string()),
                endpoint: None,
                command: None,
                timeout_ms: None,
                max_input_tokens: None,
            },
        );
        assert!(!missing_ep.is_online());
        assert!(missing_ep.complete(&valid_req).is_err());
    }

    #[test]
    fn extension_point_providers_return_structured_v03_error_and_capabilities() {
        for (alias, kind) in [
            ("oai", ModelKind::OpenaiCompat),
            ("anth", ModelKind::Anthropic),
            ("gem", ModelKind::Gemini),
            ("samp", ModelKind::Sampling),
        ] {
            let cfg = ModelConfig {
                kind,
                model: Some("ext-model".to_string()),
                endpoint: Some("https://api.example.com".to_string()),
                command: None,
                timeout_ms: Some(10_000),
                max_input_tokens: Some(16_000),
            };
            let provider =
                ProviderFactory::create_raw(alias, &cfg).expect("create extension point");
            assert_eq!(provider.kind(), kind);
            assert_eq!(provider.alias(), alias);

            let caps = provider.capabilities();
            assert!(caps.supports_json_schema);
            assert_eq!(caps.max_context_tokens, Some(16_000));
            if kind == ModelKind::Sampling {
                assert!(caps.is_agent_handoff);
                assert!(!caps.supports_streaming);
            } else {
                assert!(!caps.is_agent_handoff);
                assert!(caps.supports_streaming);
            }

            let req = LlmRequest::new(RoleName::Init, "test extension point");
            let err = provider.complete(&req).unwrap_err();
            let err_msg = err.to_string();
            assert!(
                err_msg.contains("v0.3 extension point"),
                "Expected 'v0.3 extension point' in error: {err_msg}"
            );
            let ext_err = err
                .downcast_ref::<ExtensionPointError>()
                .expect("downcast to ExtensionPointError");
            assert_eq!(ext_err.alias, alias);
            assert_eq!(ext_err.kind, kind);
        }
    }

    #[test]
    fn role_router_fallback_chain_none_mode_override_and_telemetry() {
        let mut cfg = Config::default();
        cfg.models.insert(
            "primary_broken".to_string(),
            ModelConfig {
                kind: ModelKind::OpenaiCompat, // unimplemented extension point -> fails immediately
                model: Some("gpt-ext".to_string()),
                endpoint: None,
                command: None,
                timeout_ms: None,
                max_input_tokens: None,
            },
        );
        cfg.models.insert(
            "backup_agent".to_string(),
            ModelConfig {
                kind: ModelKind::Agent,
                model: Some("harness".to_string()),
                endpoint: None,
                command: None,
                timeout_ms: None,
                max_input_tokens: None,
            },
        );

        // Role chains:
        // - init: ["primary_broken", "backup_agent"] -> falls back to backup_agent (fallback_used = true)
        // - update: ["none"] -> deterministic without calling LLM (fallback_used = false)
        // - task: ["primary_broken", "none"] -> falls back to deterministic "none" (fallback_used = true)
        // - search: ["primary_broken"] -> all aliases fail -> returns Err
        cfg.roles.init = vec!["primary_broken".to_string(), "backup_agent".to_string()];
        cfg.roles.update = vec!["none".to_string()];
        cfg.roles.task = vec!["primary_broken".to_string(), "none".to_string()];
        cfg.roles.search = vec!["primary_broken".to_string()];

        let tmp = TempDirGuard::new("router_tel");
        let jsonl_sink = Arc::new(JsonlTelemetrySink::new(tmp.path(), true));
        let mut router = RoleRouter::new(cfg, Some(jsonl_sink.clone() as Arc<dyn TelemetrySink>))
            .expect("build RoleRouter")
            .with_project_id("github.com_org_repo-12345678");

        // Also register a custom mock provider to test direct first-choice success and model_override
        router.register_provider(
            "fast_mock",
            MockProvider::succeeding("fast_mock", "{\"subsystems\": []}"),
        );

        // 1. `update` role configured with `["none"]` -> deterministic, no fallback
        let update_req = LlmRequest::new(RoleName::Update, "Update card");
        let update_outcome = router.execute(&update_req, None).unwrap();
        assert_eq!(
            update_outcome,
            RoleExecutionOutcome::Deterministic {
                role: RoleName::Update,
                fallback_used: false
            }
        );
        assert!(router.complete_role(&update_req, None).unwrap().is_none());

        // 2. `init` role configured with `["primary_broken", "backup_agent"]` -> falls back to `backup_agent`
        let init_req = LlmRequest::new(RoleName::Init, "Initialize KB");
        let init_outcome = router.execute(&init_req, None).unwrap();
        assert!(init_outcome.is_completed());
        assert!(init_outcome.fallback_used());
        let init_resp = init_outcome.into_response().unwrap();
        assert_eq!(init_resp.alias, "backup_agent");
        assert_eq!(init_resp.provider_kind, ModelKind::Agent);
        assert!(init_resp.fallback_used);
        assert!(init_resp.work_package.is_some());

        // 3. `task` role configured with `["primary_broken", "none"]` -> falls back to `"none"`
        let task_req = LlmRequest::new(RoleName::Task, "Synthesize task");
        let task_outcome = router.execute(&task_req, None).unwrap();
        assert_eq!(
            task_outcome,
            RoleExecutionOutcome::Deterministic {
                role: RoleName::Task,
                fallback_used: true
            }
        );

        // 4. `search` role configured with `["primary_broken"]` -> fails when all aliases fail
        let search_req = LlmRequest::new(RoleName::Search, "Expand query");
        assert!(router.execute(&search_req, None).is_err());

        // 5. Explicit `model_override` (`Some("fast_mock")` and `Some("none")`) overrides role chain
        let override_resp = router
            .complete_role(&search_req, Some("fast_mock"))
            .unwrap()
            .expect("completed by fast_mock override");
        assert_eq!(override_resp.alias, "fast_mock");
        assert!(!override_resp.fallback_used);

        let override_none = router.execute(&init_req, Some("none")).unwrap();
        assert!(override_none.is_deterministic());
        assert!(!override_none.fallback_used());

        // 6. Verify recorded JSONL telemetry signals and fallback counts
        let events = jsonl_sink.read_all_events().unwrap();
        assert!(!events.is_empty());
        let llm_signals: Vec<LlmSignal> = events
            .iter()
            .filter_map(|e| match &e.signal {
                TelemetrySignal::Llm(s) => Some(s.clone()),
                _ => None,
            })
            .collect();
        assert!(llm_signals
            .iter()
            .any(|s| s.role == "init" && s.alias == "backup_agent" && s.fallback_used));
        assert!(llm_signals
            .iter()
            .any(|s| s.role == "task" && s.fallback_used));

        let report = render_telemetry_report(&events, false);
        assert!(report.contains("Fallbacks triggered: 2"));
    }

    #[test]
    fn prompt_template_registry_language_parameterization_and_custom_overrides() {
        // 1. Built-in templates for all 4 roles with `language = "en"` vs `"ru"` vs `"de"`
        let reg_en = PromptTemplateRegistry::new("en");
        let reg_ru = PromptTemplateRegistry::new("ru");
        let reg_de = PromptTemplateRegistry::new("de");

        for role in [
            RoleKind::Init,
            RoleKind::Update,
            RoleKind::Task,
            RoleKind::Search,
        ] {
            let vars = PromptVars::with_facts("module A: src/lib.rs")
                .project("demo-proj")
                .query("find auth");

            let rendered_en = reg_en.render(role, &vars);
            assert_eq!(rendered_en.source, PromptTemplateSource::BuiltIn);
            assert!(rendered_en.system_prompt.contains("English (en)"));
            assert!(!rendered_en.system_prompt.contains("STRICTLY RUSSIAN"));
            assert!(!rendered_en
                .system_prompt
                .to_ascii_lowercase()
                .contains("qwen"));
            assert!(!rendered_en.prompt.to_ascii_lowercase().contains("qwen"));
            assert!(rendered_en.prompt.contains("module A: src/lib.rs"));

            let rendered_ru = reg_ru.render(role, &vars);
            assert!(rendered_ru.system_prompt.contains("Russian (ru)"));
            assert!(!rendered_ru
                .system_prompt
                .to_ascii_lowercase()
                .contains("qwen"));

            let rendered_de = reg_de.render(role, &vars);
            assert!(rendered_de.system_prompt.contains("German (de)"));
            assert!(!rendered_de.system_prompt.contains("STRICTLY RUSSIAN"));
        }

        // 2. Override templates from `<config_dir>/prompts/<role>.md` (and `.txt`) and `<repo>/.context-init/prompts/<role>.md`
        let tmp = TempDirGuard::new("prompts");
        let config_dir = tmp.path().join("global_cfg");
        let global_prompts = config_dir.join("prompts");
        let repo_root = tmp.path().join("repo");
        let repo_prompts = repo_root.join(".context-init").join("prompts");

        fs::create_dir_all(&global_prompts).unwrap();
        fs::create_dir_all(&repo_prompts).unwrap();

        // Global overrides for `init.md` and `update.txt`
        fs::write(
            global_prompts.join("init.md"),
            "GLOBAL INIT [{language}]: {facts}",
        )
        .unwrap();
        fs::write(
            global_prompts.join("update.txt"),
            "---system---\nCUSTOM GLOBAL SYSTEM ({language})\n---user---\nGLOBAL UPDATE: {facts}",
        )
        .unwrap();

        // Repo override for `init.md` (must win over global `init.md`)
        fs::write(
            repo_prompts.join("init.md"),
            "REPO INIT OVERRIDE [{language}] for {project}: {facts}",
        )
        .unwrap();

        let cfg = Config {
            language: "en".to_string(),
            ..Config::default()
        };

        let registry =
            PromptTemplateRegistry::from_config(&cfg, Some(&config_dir), Some(&repo_root))
                .expect("load prompt templates with overrides");

        let vars = PromptVars::with_facts("fact-123").project("my-service");

        // `init` uses repo override
        let init_rendered = registry.render(RoleName::Init, &vars);
        assert_eq!(
            init_rendered.source,
            PromptTemplateSource::RepoConfig(repo_prompts.join("init.md"))
        );
        assert_eq!(
            init_rendered.prompt,
            "REPO INIT OVERRIDE [en] for my-service: fact-123"
        );

        // `update` uses global `.txt` override with custom `---system---` and `---user---`
        let update_rendered = registry.render(RoleName::Update, &vars);
        assert_eq!(
            update_rendered.source,
            PromptTemplateSource::GlobalConfig(global_prompts.join("update.txt"))
        );
        assert_eq!(update_rendered.system_prompt, "CUSTOM GLOBAL SYSTEM (en)");
        assert_eq!(update_rendered.prompt, "GLOBAL UPDATE: fact-123");

        // `task` was not overridden -> remains BuiltIn
        let task_rendered = registry.render(RoleName::Task, &vars);
        assert_eq!(task_rendered.source, PromptTemplateSource::BuiltIn);

        // `to_llm_request` produces a populated LlmRequest with schema and facts
        let llm_req = registry.to_llm_request(RoleName::Init, &vars);
        assert_eq!(llm_req.role, RoleName::Init);
        assert!(llm_req.json_schema.is_some());
        assert_eq!(
            llm_req.facts,
            Some(serde_json::Value::String("fact-123".to_string()))
        );
    }

    #[test]
    fn json_schema_validator_and_router_schema_error_edge_cases() {
        // 1. Validate JSON Schema combinators, bounds, and additionalProperties
        let complex_schema = serde_json::json!({
            "type": "object",
            "required": ["count", "mode", "tags"],
            "additionalProperties": false,
            "properties": {
                "count": { "type": "integer", "minimum": 1, "maximum": 10 },
                "mode": { "enum": ["fast", "deep"] },
                "tags": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": 2,
                    "items": { "type": "string", "minLength": 2 }
                }
            }
        });

        let ok_val = serde_json::json!({
            "count": 5,
            "mode": "fast",
            "tags": ["rust", "cli"]
        });
        assert!(validate_json_schema(&complex_schema, &ok_val).is_ok());

        let extra_prop = serde_json::json!({
            "count": 5,
            "mode": "fast",
            "tags": ["rust"],
            "extra": true
        });
        assert!(validate_json_schema(&complex_schema, &extra_prop)
            .unwrap_err()
            .to_string()
            .contains("additional property"));

        let out_of_range = serde_json::json!({
            "count": 0,
            "mode": "fast",
            "tags": ["rust"]
        });
        assert!(validate_json_schema(&complex_schema, &out_of_range)
            .unwrap_err()
            .to_string()
            .contains("minimum"));

        // 2. Agent submission with stringified JSON inside `"output"` and top-level `package_id`
        let agent = AgentProvider::new("agent");
        let req = LlmRequest::new(RoleName::Update, "Update card")
            .with_json_schema(default_role_json_schema(RoleName::Update));
        let pkg = agent.build_work_package(&req);
        let wrapped_submission = serde_json::json!({
            "package_id": pkg.package_id,
            "output": "```json\n{\"id\": \"core\", \"summary\": \"Updated\", \"body\": \"Layered\"}\n```"
        });
        let validated = validate_agent_submission(&pkg, &wrapped_submission).unwrap();
        assert_eq!(validated.payload["summary"], "Updated");

        // 3. RoleRouter accumulates retries and schema_errors when primary fails schema validation
        //    and fallback succeeds
        struct SchemaFailingProvider;
        impl LlmProvider for SchemaFailingProvider {
            fn complete(&self, req: &LlmRequest) -> anyhow::Result<LlmResponse> {
                Err(anyhow::Error::new(LlmExecutionError {
                    message: "schema validation failed after retry".to_string(),
                    retries: 1,
                    schema_error: true,
                    timed_out: false,
                    budget_exceeded: false,
                    tokens_in: estimate_request_tokens(req),
                }))
            }
            fn capabilities(&self) -> ProviderCapabilities {
                ProviderCapabilities::default()
            }
            fn kind(&self) -> ModelKind {
                ModelKind::Ollama
            }
            fn alias(&self) -> &str {
                "bad_schema_primary"
            }
        }

        let mut cfg = Config::default();
        cfg.roles.update = vec![
            "bad_schema_primary".to_string(),
            "good_secondary".to_string(),
        ];
        let mem_sink = MemoryTelemetrySink::new(true);
        let mut router =
            RoleRouter::new(cfg, Some(mem_sink.clone() as Arc<dyn TelemetrySink>)).unwrap();

        router.register_provider("bad_schema_primary", SchemaFailingProvider);
        router.register_provider(
            "good_secondary",
            MockProvider::succeeding(
                "good_secondary",
                "{\"id\": \"core\", \"summary\": \"S\", \"body\": \"B\"}",
            ),
        );

        let outcome = router.execute(&req, None).unwrap();
        let resp = outcome.into_response().unwrap();
        assert_eq!(resp.alias, "good_secondary");
        assert!(resp.fallback_used);
        assert_eq!(resp.retries, 1);

        let signals = mem_sink.llm_signals();
        let last_llm = signals.last().expect("expected final router LlmSignal");
        assert_eq!(last_llm.alias, "good_secondary");
        assert!(last_llm.fallback_used);
        assert_eq!(last_llm.retries, 1);
        assert_eq!(last_llm.schema_errors, 1);
    }

    #[test]
    fn regression_nested_fences_large_stdin_timeout_single_pass_vars_and_schemars_ref() {
        // 1. Raw JSON containing triple-backtick code fences inside a string field (`RoleName::Update` body)
        let raw_json_with_inner_fence = r##"{"id": "core", "summary": "Core summary", "body": "# Core\n\n```rust\nfn main() {}\n```"}"##;
        let extracted_raw = extract_json_payload(raw_json_with_inner_fence);
        assert_eq!(extracted_raw, raw_json_with_inner_fence);
        let validated_raw = parse_and_validate_json(
            raw_json_with_inner_fence,
            Some(&default_role_json_schema(RoleName::Update)),
            10,
        )
        .expect("raw JSON with inner markdown code fence must validate");
        assert!(validated_raw["body"].as_str().unwrap().contains("```rust"));

        // 2. Leading prose + outer ```json fence + inner ```rust fence inside JSON string value
        let prose_and_nested_fence = format!(
            "Here is the updated card for {{core}}:\n```json\n{}\n```\nDone {{ok}}.",
            raw_json_with_inner_fence
        );
        let validated_nested = parse_and_validate_json(
            &prose_and_nested_fence,
            Some(&default_role_json_schema(RoleName::Update)),
            10,
        )
        .expect("fenced JSON with inner backticks and surrounding prose must validate");
        assert_eq!(validated_nested["id"], "core");
        assert!(validated_nested["body"]
            .as_str()
            .unwrap()
            .contains("```rust"));

        // 3. Unfenced JSON preceded and followed by prose containing `{braces}`
        let prose_with_braces =
            "Note for {module}: {\"id\": \"core\", \"summary\": \"S\", \"body\": \"B\"} end {tag}";
        assert_eq!(
            extract_json_payload(prose_with_braces),
            "{\"id\": \"core\", \"summary\": \"S\", \"body\": \"B\"}"
        );

        // 4. Local `$ref` resolution (`#/definitions/...`) generated by `schemars::schema_for!`
        #[derive(schemars::JsonSchema)]
        #[allow(dead_code)]
        struct EnvelopeSchema {
            subsystems: Vec<SubsystemAiSummary>,
        }
        let generated_schema =
            serde_json::to_value(schemars::schema_for!(EnvelopeSchema)).expect("schemars value");
        let valid_instance = serde_json::json!({
            "subsystems": [
                {
                    "id": "llm",
                    "name": "LLM",
                    "summary": "Multi-provider routing.",
                    "tags": ["rust"]
                }
            ]
        });
        assert!(
            validate_json_schema(&generated_schema, &valid_instance).is_ok(),
            "Valid instance must pass schemars-generated $ref schema"
        );
        let invalid_instance = serde_json::json!({
            "subsystems": [
                {
                    "id": "llm"
                    // missing required `name` and `summary` inside referenced `SubsystemAiSummary`
                }
            ]
        });
        assert!(
            validate_json_schema(&generated_schema, &invalid_instance).is_err(),
            "Invalid nested item must be rejected through $ref resolution"
        );

        // 5. `validate_agent_submission` must reject non-string `package_id` in envelope
        let agent = AgentProvider::new("agent");
        let pkg = agent.build_work_package(&LlmRequest::new(RoleName::Init, "init"));
        let non_string_pkg_id = serde_json::json!({
            "package_id": 12345,
            "result": valid_instance
        });
        assert!(
            validate_agent_submission(&pkg, &non_string_pkg_id).is_err(),
            "Numeric package_id must be rejected"
        );

        // 6. Single-pass placeholder substitution in `PromptTemplateRegistry`:
        //    `facts` containing literal `{query}` or `{json_schema}` must NOT be expanded
        let reg = PromptTemplateRegistry::new("en");
        let literal_facts = "fn test() { let tpl = \"{query} and {json_schema}\"; }";
        let vars = PromptVars::with_facts(literal_facts)
            .project("proj-{facts}")
            .query("SEARCH_TERM_XYZ");
        let rendered = reg.render(RoleName::Task, &vars);
        assert!(
            rendered.prompt.contains(literal_facts),
            "Literal placeholders inside `facts` must not be expanded: {}",
            rendered.prompt
        );
        assert!(
            rendered.prompt.contains("Project: proj-{facts}"),
            "Literal `{{facts}}` inside `project` must not be expanded"
        );

        // 7. `RetryDecorator` must NOT retry `ExtensionPointError` and must preserve its error type
        let oai = OpenAiCompatProvider::from_config(
            "oai_ext",
            &ModelConfig {
                kind: ModelKind::OpenaiCompat,
                model: Some("gpt-4o".to_string()),
                endpoint: None,
                command: None,
                timeout_ms: None,
                max_input_tokens: None,
            },
        );
        let retry_ext = RetryDecorator::new(oai, 3);
        let ext_err = retry_ext
            .complete(&LlmRequest::new(RoleName::Init, "hi"))
            .unwrap_err();
        assert!(
            ext_err.downcast_ref::<ExtensionPointError>().is_some(),
            "ExtensionPointError must not be wrapped/retried by RetryDecorator"
        );

        // 8. `RoleRouter`: empty role chain defaults to deterministic `"none"`, and a failing chain
        //    ending in a missing alias still emits failure telemetry
        let mut cfg = Config::default();
        cfg.roles.init = vec![];
        cfg.roles.search = vec!["failing_primary".to_string(), "missing_alias".to_string()];
        let sink = MemoryTelemetrySink::new(true);
        let mut router =
            RoleRouter::new(cfg, Some(sink.clone() as Arc<dyn TelemetrySink>)).unwrap();
        router.register_provider(
            "failing_primary",
            MockProvider::failing("failing_primary", 5, ""),
        );

        let empty_chain_outcome = router
            .execute(&LlmRequest::new(RoleName::Init, "init"), None)
            .unwrap();
        assert_eq!(
            empty_chain_outcome,
            RoleExecutionOutcome::Deterministic {
                role: RoleName::Init,
                fallback_used: false,
            }
        );

        let search_err = router
            .execute(&LlmRequest::new(RoleName::Search, "query"), None)
            .unwrap_err();
        assert!(search_err.to_string().contains("missing_alias"));
        let signals = sink.llm_signals();
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].role, "search");
        assert!(signals[0].fallback_used);

        // 9. `CommandProvider`: large (>128 KB) prompt over stdin must not deadlock when child
        //    exits without reading stdin OR hangs and hits `timeout_ms`
        #[cfg(not(windows))]
        {
            let large_prompt = "A".repeat(200 * 1024); // 200 KB (> 64 KB Linux pipe capacity)

            // 9a. Child exits immediately without reading stdin (`sh -c "printf ok"`)
            let early_exit_cmd = CommandProvider::new(
                "early_exit",
                vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "printf '{\"subsystems\": []}'".to_string(),
                ],
            )
            .unwrap();
            let large_req = LlmRequest::new(RoleName::Init, large_prompt.clone());
            let early_resp = early_exit_cmd
                .complete(&large_req)
                .expect("large stdin with early-exit child must succeed");
            assert_eq!(early_resp.content, "{\"subsystems\": []}");

            // 9b. Child sleeps without reading stdin (`sleep 5`) with 60ms timeout -> must time out quickly without deadlocking
            let hanging_cmd =
                CommandProvider::new("hang_stdin", vec!["sleep".to_string(), "5".to_string()])
                    .unwrap()
                    .with_timeout_ms(60);
            let start = std::time::Instant::now();
            let hang_err = hanging_cmd.complete(&large_req).unwrap_err();
            let elapsed = start.elapsed();
            assert!(
                inspect_execution_error(&hang_err).is_some_and(|m| m.timed_out),
                "Expected timed_out error, got: {hang_err}"
            );
            assert!(
                elapsed < Duration::from_secs(2),
                "Large stdin timeout took too long ({elapsed:?}), pipe write blocked timeout loop"
            );

            // 9c. Single-pass argument placeholder substitution in CommandProvider
            let arg_prov = CommandProvider::new(
                "arg_pass",
                vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "printf '%s|%s' \"$1\" \"$2\"".to_string(),
                    "--".to_string(),
                    "{prompt}".to_string(),
                    "{model}".to_string(),
                ],
            )
            .unwrap()
            .with_model("ACTUAL_MODEL");
            let arg_req = LlmRequest::new(RoleName::Task, " literal {model} in prompt ");
            let arg_resp = arg_prov.complete(&arg_req).unwrap();
            assert_eq!(arg_resp.content, "literal {model} in prompt |ACTUAL_MODEL");
        }
    }
}
