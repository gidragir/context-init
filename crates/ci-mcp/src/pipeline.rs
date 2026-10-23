//! Shared init/update orchestration for the CLI and MCP: detect → optional LLM
//! enrichment (`ci-llm`) → generate/update the knowledge base (`ci-core`).

use anyhow::Result;
use std::path::PathBuf;

use ci_core::analysis::generator::{self, GenerateOptions};
use ci_core::analysis::subsystems::build_deterministic;
use ci_core::analysis::{detector, ProjectAnalysis};
use ci_core::storage::SubsystemInfo;
use ci_llm::enrich::{enrich_subsystems, EnrichOutcome};
use ci_llm::{PromptTemplateRegistry, RoleName, RoleRouter};

use crate::handlers::ResolvedProject;

/// Options shared by [`run_init`] and [`run_update`].
#[derive(Debug, Clone, Copy, Default)]
pub struct PipelineOptions<'a> {
    /// Deep AST scan (symbols).
    pub deep: bool,
    /// Overwrite an existing knowledge base (`init` only).
    pub force: bool,
    /// Analyze and validate only; write nothing (`init` only).
    pub dry_run: bool,
    /// Refresh the navigation section of the repo's `AGENTS.md`.
    pub with_agents_md: bool,
    /// `--model` / MCP `model` override for the role chain (D16).
    pub model_override: Option<&'a str>,
}

/// Result of a pipeline run.
#[derive(Debug, Clone)]
pub struct PipelineOutcome {
    pub analysis: ProjectAnalysis,
    pub subsystems: Vec<SubsystemInfo>,
    /// Knowledge dir that was written (or would be, for dry runs).
    pub knowledge_dir: PathBuf,
    pub enrich: EnrichOutcome,
    /// Subsystems added (`update`) or created (`init`).
    pub added: usize,
    /// Existing subsystems refreshed (`update` only).
    pub updated: usize,
}

/// Builds a fresh knowledge base in central storage (D1, D23).
pub fn run_init(
    project: &ResolvedProject,
    router: &RoleRouter,
    opts: &PipelineOptions,
) -> Result<PipelineOutcome> {
    let (analysis, subsystems, enrich) = analyze(project, router, RoleName::Init, opts)?;
    let knowledge_dir = project
        .storage_paths
        .project_knowledge_dir(&project.project_id);
    let gen_opts = GenerateOptions {
        force: opts.force,
        dry_run: opts.dry_run,
        with_agents_md: opts.with_agents_md,
    };
    generator::generate(&analysis, &subsystems, &knowledge_dir, &gen_opts)?;
    Ok(PipelineOutcome {
        added: subsystems.len(),
        updated: 0,
        analysis,
        subsystems,
        knowledge_dir,
        enrich,
    })
}

/// Re-detects the project and merges the result into the active knowledge base.
pub fn run_update(
    project: &ResolvedProject,
    router: &RoleRouter,
    opts: &PipelineOptions,
) -> Result<PipelineOutcome> {
    let (analysis, subsystems, enrich) = analyze(project, router, RoleName::Update, opts)?;
    let knowledge_dir = project.knowledge_dir.clone();
    let (added, updated) =
        generator::update(&analysis, &subsystems, &knowledge_dir, opts.with_agents_md)?;
    Ok(PipelineOutcome {
        analysis,
        subsystems,
        knowledge_dir,
        enrich,
        added,
        updated,
    })
}

/// Hint about a legacy in-repo knowledge base: either it is still the one being read
/// (`central_active == false`), or central storage took over and the copy is stale.
pub fn legacy_hint(project: &ResolvedProject, central_active: bool) -> Option<String> {
    let legacy = project.root.join(".agents").join("knowledge");
    let present =
        legacy.join("L0_index.json").is_file() || project.knowledge_dir.starts_with(&legacy);
    if !present {
        return None;
    }
    Some(if central_active {
        format!(
            "The in-repo knowledge base at {} is no longer read (central storage is used); you can delete it.",
            legacy.display()
        )
    } else {
        format!(
            "Reading the legacy in-repo knowledge base at {}. Run `context-init migrate` to move it to central storage.",
            legacy.display()
        )
    })
}

fn analyze(
    project: &ResolvedProject,
    router: &RoleRouter,
    role: RoleName,
    opts: &PipelineOptions,
) -> Result<(ProjectAnalysis, Vec<SubsystemInfo>, EnrichOutcome)> {
    let analysis = detector::analyze_project(
        &project.root,
        opts.deep,
        project.config.integrations.resolve_rtk_active(),
    )?;
    let mut subsystems = build_deterministic(&analysis);
    let prompts = PromptTemplateRegistry::new(project.config.language.clone());
    let enrich = enrich_subsystems(
        router,
        &prompts,
        role,
        &analysis,
        &mut subsystems,
        opts.model_override,
    );
    Ok((analysis, subsystems, enrich))
}
