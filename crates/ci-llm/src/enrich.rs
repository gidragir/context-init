//! Optional LLM enrichment of deterministic subsystems via `RoleRouter` (D16).
//! Replaces the legacy ad-hoc Ollama path: Ollama is just a `[models.*]` provider.

use ci_core::analysis::generator::SummaryUpdate;
use ci_core::analysis::ProjectAnalysis;
use ci_core::storage::SubsystemInfo;

use crate::providers::parse_subsystem_summaries;
use crate::{
    AgentWorkPackage, PromptTemplateRegistry, PromptVars, RoleExecutionOutcome, RoleName,
    RoleRouter,
};

/// Result of [`enrich_subsystems`]. Enrichment never fails the pipeline.
#[derive(Debug, Clone, PartialEq)]
pub enum EnrichOutcome {
    /// The role resolved to deterministic `none`; nothing changed.
    Skipped,
    /// The model answered with these summaries for known subsystems; the caller applies them
    /// (`generator::apply_summaries`) with `model` as the recorded alias.
    Enriched {
        updates: Vec<SummaryUpdate>,
        model: String,
    },
    /// The role resolved to an `agent` model: the harness must answer this package via
    /// `context_submit`.
    PendingAgent(AgentWorkPackage),
    /// The provider chain failed; nothing is to be applied.
    Failed { warning: String },
}

/// Asks the model chain for `role` to improve subsystem summaries and tags. Returns the
/// answer as updates keyed by subsystem id without changing `subsystems`: unknown ids and
/// items without a summary are dropped, so the set of subsystems always stays the
/// detector's.
pub fn enrich_subsystems(
    router: &RoleRouter,
    prompts: &PromptTemplateRegistry,
    role: RoleName,
    analysis: &ProjectAnalysis,
    subsystems: &[SubsystemInfo],
    model_override: Option<&str>,
) -> EnrichOutcome {
    let schema = crate::default_role_json_schema(role);
    let vars = PromptVars::with_facts(subsystem_facts(analysis, subsystems))
        .project(analysis.name.clone())
        .json_schema(schema.to_string());
    let req = prompts.to_llm_request(role, &vars);

    let resp = match router.execute(&req, model_override) {
        Ok(RoleExecutionOutcome::Deterministic { .. }) => return EnrichOutcome::Skipped,
        Ok(RoleExecutionOutcome::Completed(resp)) => resp,
        Err(e) => {
            return EnrichOutcome::Failed {
                warning: format!("{e:#}"),
            }
        }
    };
    if let Some(pkg) = resp.work_package {
        return EnrichOutcome::PendingAgent(pkg);
    }
    match parse_subsystem_summaries(&resp.content) {
        Ok(items) => {
            let mut updates: Vec<SummaryUpdate> = Vec::new();
            for item in items {
                let known = subsystems.iter().any(|s| s.id == item.id);
                // A blank summary carries nothing to attribute to the model.
                if !known || item.summary.trim().is_empty() {
                    continue;
                }
                updates.push(SummaryUpdate {
                    id: item.id,
                    name: (!item.name.trim().is_empty()).then_some(item.name),
                    summary: item.summary,
                    tags: (!item.tags.is_empty()).then_some(item.tags),
                });
            }
            EnrichOutcome::Enriched {
                updates,
                model: resp.alias,
            }
        }
        Err(e) => EnrichOutcome::Failed {
            warning: format!("{e:#}"),
        },
    }
}

/// One-line CLI notice for an enrichment outcome (`None` when there is nothing to say).
/// The CLI has no harness to answer an agent package, so it keeps deterministic results.
pub fn cli_outcome_message(outcome: &EnrichOutcome) -> Option<String> {
    match outcome {
        EnrichOutcome::Skipped => None,
        EnrichOutcome::Enriched { updates, model } => Some(format!(
            "LLM enrichment ({model}) updated {} subsystem(s).",
            updates.len()
        )),
        EnrichOutcome::PendingAgent(_) => Some(
            "Model kind \"agent\" is answered by an AI harness over MCP (context_init); \
             the CLI keeps deterministic summaries."
                .to_string(),
        ),
        EnrichOutcome::Failed { warning } => Some(format!(
            "LLM enrichment failed, keeping deterministic summaries: {warning}"
        )),
    }
}

/// Grounded facts for the prompt: the existing subsystem ids (which the answer must
/// reuse) with their key files, entrypoints and symbols.
fn subsystem_facts(analysis: &ProjectAnalysis, subsystems: &[SubsystemInfo]) -> String {
    let mut facts = format!(
        "Project: {}\nType: {}\nDescription: {}\nUse exactly these subsystem ids:\n",
        analysis.name,
        analysis.project_type.display_name(),
        analysis.description
    );
    for s in subsystems {
        let files: Vec<&str> = s.files.iter().take(5).map(|f| f.path.as_str()).collect();
        let symbols: Vec<&str> = s.key_symbols.iter().take(5).map(String::as_str).collect();
        facts.push_str(&format!(
            "- id `{}` ({}): files [{}]; entrypoints [{}]; symbols [{}]\n",
            s.id,
            s.name,
            files.join(", "),
            s.entrypoints.join(", "),
            symbols.join(", ")
        ));
    }
    facts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{LlmProvider, LlmRequest, LlmResponse, ProviderCapabilities};
    use crate::providers::ProviderRegistry;
    use crate::{PromptTemplateRegistry, RoleName, RoleRouter};
    use ci_core::analysis::ProjectAnalysis;
    use ci_core::config::{Config, ModelConfig, ModelKind};
    use ci_core::storage::{ProjectCommands, ProjectType, SubsystemInfo};
    use std::path::PathBuf;

    struct Scripted {
        reply: Result<String, String>,
    }

    impl LlmProvider for Scripted {
        fn complete(&self, _req: &LlmRequest) -> anyhow::Result<LlmResponse> {
            let content = self.reply.clone().map_err(anyhow::Error::msg)?;
            Ok(LlmResponse {
                content,
                model: "scripted".to_string(),
                provider_kind: ModelKind::Command,
                alias: "scripted".to_string(),
                tokens_in: 1,
                tokens_out: 1,
                latency_ms: 0,
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
                max_context_tokens: None,
            }
        }
        fn kind(&self) -> ModelKind {
            ModelKind::Command
        }
        fn alias(&self) -> &str {
            "scripted"
        }
    }

    fn router_with(reply: Result<&str, &str>) -> RoleRouter {
        let mut cfg = Config::default();
        cfg.roles.init = vec!["scripted".to_string()];
        let mut registry = ProviderRegistry::new();
        registry.register(
            "scripted",
            Scripted {
                reply: reply.map(str::to_string).map_err(str::to_string),
            },
        );
        RoleRouter::with_registry(cfg, registry, None)
    }

    fn analysis() -> ProjectAnalysis {
        ProjectAnalysis {
            name: "demo".to_string(),
            root_dir: PathBuf::from("/tmp/demo"),
            project_type: ProjectType::Rust,
            description: String::new(),
            detected_modules: Vec::new(),
            root_entrypoints: Vec::new(),
            commands: ProjectCommands::default(),
            workspace_members: Vec::new(),
            rules: Vec::new(),
        }
    }

    fn subs() -> Vec<SubsystemInfo> {
        ["core", "api"]
            .iter()
            .map(|id| SubsystemInfo {
                id: id.to_string(),
                name: id.to_string(),
                summary: format!("deterministic {id}"),
                l1_path: format!("subsystems/{id}.md"),
                tags: vec![],
                entrypoints: vec![],
                key_files: vec![],
                files: vec![],
                key_symbols: vec![],
                test_command: None,
                tokens: 0,
                parent: None,
                summary_source: ci_core::SummarySource::Heuristic,
                summary_model: None,
                needs_review: false,
            })
            .collect()
    }

    fn prompts() -> PromptTemplateRegistry {
        PromptTemplateRegistry::new("en")
    }

    #[test]
    fn enrich_none_role_is_skipped() {
        let router = RoleRouter::with_registry(Config::default(), ProviderRegistry::new(), None);
        let s = subs();
        let out = enrich_subsystems(&router, &prompts(), RoleName::Init, &analysis(), &s, None);
        assert_eq!(out, EnrichOutcome::Skipped);
        assert_eq!(s, subs());
    }

    #[test]
    fn enrich_merges_summaries_by_id() {
        let router = router_with(Ok(
            r#"{"subsystems":[{"id":"core","name":"Core","summary":"LLM core","tags":["rust"]},{"id":"ghost","name":"G","summary":"x","tags":[]}]}"#,
        ));
        let s = subs();
        let out = enrich_subsystems(&router, &prompts(), RoleName::Init, &analysis(), &s, None);
        assert_eq!(
            out,
            EnrichOutcome::Enriched {
                updates: vec![SummaryUpdate {
                    id: "core".to_string(),
                    name: Some("Core".to_string()),
                    summary: "LLM core".to_string(),
                    tags: Some(vec!["rust".to_string()]),
                }],
                model: "scripted".to_string(),
            }
        );
        // The subsystems themselves are never mutated.
        assert_eq!(s, subs());
    }

    #[test]
    fn enrich_provider_error_returns_failed_and_keeps_subsystems() {
        let router = router_with(Err("connection refused"));
        let s = subs();
        let out = enrich_subsystems(&router, &prompts(), RoleName::Init, &analysis(), &s, None);
        assert!(
            matches!(out, EnrichOutcome::Failed { ref warning } if warning.contains("connection refused")),
            "{out:?}"
        );
        assert_eq!(s, subs());
    }

    #[test]
    fn enrich_agent_kind_returns_pending_package() {
        let mut cfg = Config::default();
        cfg.models.insert(
            "harness".to_string(),
            ModelConfig {
                kind: ModelKind::Agent,
                model: None,
                endpoint: None,
                command: None,
                timeout_ms: None,
                max_input_tokens: None,
            },
        );
        cfg.roles.init = vec!["harness".to_string()];
        let router = RoleRouter::new(cfg, None).unwrap();
        let s = subs();
        let out = enrich_subsystems(&router, &prompts(), RoleName::Init, &analysis(), &s, None);
        match out {
            EnrichOutcome::PendingAgent(pkg) => assert_eq!(pkg.role, RoleName::Init),
            other => panic!("expected PendingAgent, got {other:?}"),
        }
        assert_eq!(s, subs());
    }

    #[test]
    fn enrich_agent_kind_from_cli_falls_back_with_warning() {
        let pkg = crate::AgentWorkPackage {
            package_id: "pkg-h-init-1".to_string(),
            role: RoleName::Init,
            facts: serde_json::json!({}),
            instruction: String::new(),
            json_schema: crate::default_role_json_schema(RoleName::Init),
            alias: "h".to_string(),
            deep: false,
        };
        let msg = cli_outcome_message(&EnrichOutcome::PendingAgent(pkg)).unwrap();
        assert!(msg.contains("agent") && msg.contains("MCP"), "{msg}");
        assert_eq!(cli_outcome_message(&EnrichOutcome::Skipped), None);
    }
}
