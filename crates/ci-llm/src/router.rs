//! Role-based LLM routing (`RoleRouter`) with fallback chains, per-call `model_override`,
//! and `"none"` deterministic mode (D16, D29).

use crate::decorators::TelemetryDecorator;
use crate::provider::{
    estimate_request_tokens, inspect_execution_error, LlmProvider, LlmRequest, LlmResponse,
    RoleName,
};
use crate::providers::ProviderRegistry;
use anyhow::{bail, Result};
use ci_core::config::{Config, ROLE_NONE};
use ci_core::telemetry::{LlmSignal, TelemetryEvent, TelemetrySignal, TelemetrySink};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Outcome of routing and executing a role through `RoleRouter` (D16).
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum RoleExecutionOutcome {
    /// Deterministic mode (`"none"` alias reached without invoking external LLM, or after fallback to `"none"`).
    Deterministic {
        /// Executed role.
        role: RoleName,
        /// `true` if a preceding model alias in the role chain failed before reaching `"none"`.
        fallback_used: bool,
    },
    /// Completed by an LLM or agent provider in the role chain.
    Completed(LlmResponse),
}

impl RoleExecutionOutcome {
    /// Returns `true` if the role resolved to deterministic `"none"` mode.
    pub fn is_deterministic(&self) -> bool {
        matches!(self, Self::Deterministic { .. })
    }

    /// Returns `true` if an LLM/agent provider produced a response.
    pub fn is_completed(&self) -> bool {
        matches!(self, Self::Completed(_))
    }

    /// Returns whether a fallback in the role chain was triggered.
    pub fn fallback_used(&self) -> bool {
        match self {
            Self::Deterministic { fallback_used, .. } => *fallback_used,
            Self::Completed(resp) => resp.fallback_used,
        }
    }

    /// Consumes the outcome and returns `Some(LlmResponse)` if completed by a provider, or `None` if deterministic.
    pub fn into_response(self) -> Option<LlmResponse> {
        match self {
            Self::Deterministic { .. } => None,
            Self::Completed(resp) => Some(resp),
        }
    }

    /// Returns a reference to the `LlmResponse` if completed by a provider, or `None` if deterministic.
    pub fn as_response(&self) -> Option<&LlmResponse> {
        match self {
            Self::Deterministic { .. } => None,
            Self::Completed(resp) => Some(resp),
        }
    }
}

/// Role router that resolves `init`, `update`, `task`, and `search` requests across configured
/// fallback chains (`[roles]`) with optional per-call `model_override` and telemetry (D16, D29).
pub struct RoleRouter {
    config: Config,
    registry: ProviderRegistry,
    telemetry: Option<Arc<dyn TelemetrySink>>,
    project_id: String,
}

impl RoleRouter {
    /// Creates a new `RoleRouter` from `Config` and an optional `TelemetrySink`.
    pub fn new(config: Config, telemetry: Option<Arc<dyn TelemetrySink>>) -> Result<Self> {
        let registry = ProviderRegistry::from_config(&config)?;
        Ok(Self {
            config,
            registry,
            telemetry,
            project_id: "default-project".to_string(),
        })
    }

    /// Creates a `RoleRouter` with a pre-built `ProviderRegistry` (useful for injecting custom/mock providers).
    pub fn with_registry(
        config: Config,
        registry: ProviderRegistry,
        telemetry: Option<Arc<dyn TelemetrySink>>,
    ) -> Self {
        Self {
            config,
            registry,
            telemetry,
            project_id: "default-project".to_string(),
        }
    }

    /// Sets the `project_id` (or hash) attached to emitted `TelemetryEvent`s.
    pub fn with_project_id(mut self, project_id: impl Into<String>) -> Self {
        self.project_id = project_id.into();
        self
    }

    /// Registers or overrides a provider under `alias` in the router's internal registry.
    pub fn register_provider<P: LlmProvider + 'static>(
        &mut self,
        alias: impl Into<String>,
        provider: P,
    ) {
        self.registry.register(alias, provider);
    }

    /// Returns a reference to the underlying `Config`.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Returns a reference to the underlying `ProviderRegistry`.
    pub fn registry(&self) -> &ProviderRegistry {
        &self.registry
    }

    /// Resolves the ordered alias chain for `role`, taking into account an optional per-call `model_override`.
    pub fn resolve_chain(&self, role: RoleName, model_override: Option<&str>) -> Vec<String> {
        if let Some(ovr) = model_override {
            let chain: Vec<String> = ovr
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)
                .collect();
            if !chain.is_empty() {
                return chain;
            }
        }

        let configured = self.config.roles.get(role);
        let chain: Vec<String> = configured
            .iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .collect();

        if chain.is_empty() {
            vec![ROLE_NONE.to_string()]
        } else {
            chain
        }
    }

    /// Executes `req` across the resolved fallback chain for `req.role`, returning `RoleExecutionOutcome`.
    ///
    /// - If `"none"` is encountered, returns `RoleExecutionOutcome::Deterministic { role, fallback_used }`
    ///   without invoking any external LLM.
    /// - If a provider fails, advances to the next alias in the chain and marks `fallback_used = true`
    ///   in both `LlmResponse` and `LlmSignal`.
    pub fn execute(
        &self,
        req: &LlmRequest,
        model_override: Option<&str>,
    ) -> Result<RoleExecutionOutcome> {
        let chain = self.resolve_chain(req.role, model_override);
        let mut errors: Vec<String> = Vec::new();
        let mut last_failed_info: Option<(String, String, usize)> = None;
        let mut accumulated_retries = 0u32;
        let mut accumulated_schema_errors = 0usize;

        for (idx, alias) in chain.iter().enumerate() {
            let trimmed_alias = alias.trim();
            let fallback_used = idx > 0;

            if trimmed_alias.eq_ignore_ascii_case(ROLE_NONE) {
                if fallback_used {
                    if let Some(ref sink) = self.telemetry {
                        let (failed_alias, failed_kind, tokens_in) =
                            last_failed_info.take().unwrap_or_else(|| {
                                (
                                    ROLE_NONE.to_string(),
                                    ROLE_NONE.to_string(),
                                    estimate_request_tokens(req),
                                )
                            });
                        let signal = LlmSignal {
                            alias: failed_alias,
                            kind: failed_kind,
                            role: req.role.as_str().to_string(),
                            input_tokens: tokens_in,
                            output_tokens: 0,
                            fallback_used: true,
                            schema_errors: accumulated_schema_errors,
                            retries: accumulated_retries as usize,
                        };
                        let _ = sink.record(&TelemetryEvent::new(
                            &self.project_id,
                            TelemetrySignal::Llm(signal),
                        ));
                    }
                }
                return Ok(RoleExecutionOutcome::Deterministic {
                    role: req.role,
                    fallback_used,
                });
            }

            let is_last_in_chain = idx + 1 == chain.len();

            let Some(provider) = self.registry.get(trimmed_alias) else {
                let err_msg = format!(
                    "Unknown model alias '{}' in role '{}' chain",
                    trimmed_alias, req.role
                );
                let tokens_in = estimate_request_tokens(req);
                if last_failed_info.is_none() {
                    last_failed_info =
                        Some((trimmed_alias.to_string(), "unknown".to_string(), tokens_in));
                }
                if is_last_in_chain {
                    if let Some(ref sink) = self.telemetry {
                        let (failed_alias, failed_kind, failed_tokens_in) =
                            last_failed_info.take().unwrap_or_else(|| {
                                (trimmed_alias.to_string(), "unknown".to_string(), tokens_in)
                            });
                        let signal = LlmSignal {
                            alias: failed_alias,
                            kind: failed_kind,
                            role: req.role.as_str().to_string(),
                            input_tokens: failed_tokens_in,
                            output_tokens: 0,
                            fallback_used,
                            schema_errors: accumulated_schema_errors,
                            retries: accumulated_retries as usize,
                        };
                        let _ = sink.record(&TelemetryEvent::new(
                            &self.project_id,
                            TelemetrySignal::Llm(signal),
                        ));
                    }
                }
                errors.push(err_msg);
                continue;
            };

            let provider_kind_str = provider.kind().as_str().to_string();

            match provider.complete(req) {
                Ok(mut resp) => {
                    if resp.tokens_in == 0 {
                        resp.tokens_in = estimate_request_tokens(req);
                    }
                    if resp.tokens_out == 0 {
                        resp.tokens_out = crate::provider::estimate_tokens(&resp.content);
                    }
                    if fallback_used {
                        resp.fallback_used = true;
                        resp.retries = resp.retries.saturating_add(accumulated_retries);
                    }
                    if let Some(ref sink) = self.telemetry {
                        if accumulated_schema_errors == 0 {
                            let tel_provider = TelemetryDecorator::new(
                                PrecomputedProvider {
                                    resp: resp.clone(),
                                    caps: provider.capabilities(),
                                },
                                Arc::clone(sink),
                            )
                            .with_project_id(&self.project_id)
                            .with_fallback_used(fallback_used);
                            let resp = tel_provider.complete(req)?;
                            return Ok(RoleExecutionOutcome::Completed(resp));
                        } else {
                            let signal = LlmSignal {
                                alias: resp.alias.clone(),
                                kind: resp.provider_kind.as_str().to_string(),
                                role: req.role.as_str().to_string(),
                                input_tokens: resp.tokens_in,
                                output_tokens: resp.tokens_out,
                                fallback_used: resp.fallback_used,
                                schema_errors: accumulated_schema_errors,
                                retries: resp.retries as usize,
                            };
                            let _ = sink.record(&TelemetryEvent::new(
                                &self.project_id,
                                TelemetrySignal::Llm(signal),
                            ));
                        }
                    }
                    return Ok(RoleExecutionOutcome::Completed(resp));
                }
                Err(err) => {
                    let (retries, schema_err, tokens_in) =
                        if let Some(meta) = inspect_execution_error(&err) {
                            (meta.retries, meta.schema_error, meta.tokens_in)
                        } else {
                            let msg_lower = err.to_string().to_ascii_lowercase();
                            let is_schema =
                                msg_lower.contains("schema") || msg_lower.contains("invalid json");
                            (0, is_schema, estimate_request_tokens(req))
                        };

                    accumulated_retries = accumulated_retries.saturating_add(retries);
                    accumulated_schema_errors =
                        accumulated_schema_errors.saturating_add(usize::from(schema_err));

                    last_failed_info = Some((
                        trimmed_alias.to_string(),
                        provider_kind_str.clone(),
                        tokens_in,
                    ));

                    if is_last_in_chain {
                        if let Some(ref sink) = self.telemetry {
                            let signal = LlmSignal {
                                alias: trimmed_alias.to_string(),
                                kind: provider_kind_str,
                                role: req.role.as_str().to_string(),
                                input_tokens: tokens_in,
                                output_tokens: 0,
                                fallback_used,
                                schema_errors: accumulated_schema_errors,
                                retries: accumulated_retries as usize,
                            };
                            let _ = sink.record(&TelemetryEvent::new(
                                &self.project_id,
                                TelemetrySignal::Llm(signal),
                            ));
                        }
                    }

                    errors.push(format!("{trimmed_alias}: {err}"));
                }
            }
        }

        bail!(
            "All model aliases in role '{}' chain {:?} failed: {}",
            req.role,
            chain,
            errors.join(" | ")
        )
    }

    /// Convenience wrapper around `execute` that returns `Ok(None)` when the role resolves to
    /// deterministic `"none"` mode and `Ok(Some(LlmResponse))` when completed by a provider.
    pub fn complete_role(
        &self,
        req: &LlmRequest,
        model_override: Option<&str>,
    ) -> Result<Option<LlmResponse>> {
        Ok(self.execute(req, model_override)?.into_response())
    }
}

struct PrecomputedProvider {
    resp: LlmResponse,
    caps: crate::provider::ProviderCapabilities,
}

impl LlmProvider for PrecomputedProvider {
    fn complete(&self, _req: &LlmRequest) -> Result<LlmResponse> {
        Ok(self.resp.clone())
    }

    fn capabilities(&self) -> crate::provider::ProviderCapabilities {
        self.caps
    }

    fn kind(&self) -> ci_core::config::ModelKind {
        self.resp.provider_kind
    }

    fn alias(&self) -> &str {
        &self.resp.alias
    }
}
