//! Shared init/update orchestration for the CLI and MCP: detect → optional LLM
//! enrichment (`ci-llm`) → generate/update the knowledge base (`ci-core`).

use anyhow::Result;
use std::path::PathBuf;

use ci_core::analysis::generator::{self, GenerateOptions};
use ci_core::analysis::subsystems::build_deterministic;
use ci_core::analysis::{detector, ProjectAnalysis};
use ci_core::config::StorageMode;
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

/// Builds a fresh knowledge base in the configured storage (`config.storage`).
pub fn run_init(
    project: &ResolvedProject,
    router: &RoleRouter,
    opts: &PipelineOptions,
) -> Result<PipelineOutcome> {
    let (analysis, subsystems, enrich) = analyze(project, router, RoleName::Init, opts)?;
    let knowledge_dir = project.knowledge_dir.clone();
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

/// Hint for a project whose active knowledge dir has no index while the other storage mode
/// holds one (the mode was switched or the base was created under a different setting).
pub fn storage_mismatch_hint(project: &ResolvedProject) -> Option<String> {
    if project.knowledge_dir.join("L0_index.json").is_file() {
        return None;
    }
    let other = match project.storage {
        StorageMode::Repo => StorageMode::Central,
        StorageMode::Central => StorageMode::Repo,
    };
    let other_dir = project.knowledge_dir_for(other);
    other_dir.join("L0_index.json").is_file().then(|| {
        format!(
            "Knowledge base found at {}, but storage = \"{}\". Run `context-init migrate --to {}` or set storage = \"{}\" in .context-init.toml.",
            other_dir.display(),
            project.storage.as_str(),
            other.as_str(),
            other.as_str()
        )
    })
}

fn analyze(
    project: &ResolvedProject,
    router: &RoleRouter,
    role: RoleName,
    opts: &PipelineOptions,
) -> Result<(ProjectAnalysis, Vec<SubsystemInfo>, EnrichOutcome)> {
    let analysis = detector::analyze_project_with(
        &project.root,
        &detector::AnalyzeOptions {
            deep: opts.deep,
            with_rtk: project.config.integrations.resolve_rtk_active(),
            split_threshold_tokens: project.config.split_threshold_tokens,
        },
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
