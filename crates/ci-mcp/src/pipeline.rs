//! Shared init/update orchestration for the CLI and MCP: detect → optional LLM
//! enrichment (`ci-llm`) → generate/update the knowledge base (`ci-core`).

use anyhow::{bail, Result};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ci_core::analysis::generator::{self, GenerateOptions};
use ci_core::analysis::subsystems::build_deterministic;
use ci_core::analysis::{detector, ProjectAnalysis};
use ci_core::config::{Config, StorageMode, ROLE_NONE};
use ci_core::storage::{L0Index, SubsystemInfo};
use ci_core::SummarySource;
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
    /// `update` found no subsystem that needs LLM enrichment (no model was consulted).
    pub nothing_to_enrich: bool,
}

/// Builds a fresh knowledge base in the configured storage (`config.storage`).
pub fn run_init(
    project: &ResolvedProject,
    router: &RoleRouter,
    opts: &PipelineOptions,
) -> Result<PipelineOutcome> {
    let (analysis, mut subsystems, enrich, _) =
        analyze(project, router, RoleName::Init, opts, None)?;
    let knowledge_dir = project.knowledge_dir.clone();
    let agents_md = opts.with_agents_md.then_some(project.storage);
    let gen_opts = GenerateOptions {
        force: opts.force,
        dry_run: opts.dry_run,
        agents_md,
    };
    generator::generate(&analysis, &subsystems, &knowledge_dir, &gen_opts)?;
    let enrich = if opts.dry_run {
        enrich
    } else {
        apply_enrichment(&analysis, &knowledge_dir, enrich, agents_md)
    };
    if matches!(enrich, EnrichOutcome::Enriched { .. }) {
        // Report what the knowledge base now holds, LLM summaries included.
        if let Ok(index) = L0Index::load_from_file(&knowledge_dir.join("L0_index.json")) {
            subsystems = index.subsystems;
        }
    }
    Ok(PipelineOutcome {
        added: subsystems.len(),
        updated: 0,
        nothing_to_enrich: false,
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
    let existing = project.load_l0_index().unwrap_or_default();
    let (analysis, subsystems, enrich, nothing_to_enrich) =
        analyze(project, router, RoleName::Update, opts, Some(&existing))?;
    let knowledge_dir = project.knowledge_dir.clone();
    let agents_md = opts.with_agents_md.then_some(project.storage);
    let (added, updated) = generator::update(&analysis, &subsystems, &knowledge_dir, agents_md)?;
    let enrich = apply_enrichment(&analysis, &knowledge_dir, enrich, agents_md);
    Ok(PipelineOutcome {
        analysis,
        subsystems,
        knowledge_dir,
        enrich,
        added,
        updated,
        nothing_to_enrich,
    })
}

/// Full drift report shared by the CLI `check` and the MCP `context_check`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FullDriftReport {
    pub has_drift: bool,
    pub missing_in_fs: Vec<String>,
    pub untracked_in_knowledge: Vec<String>,
    pub up_to_date_count: usize,
    pub git_stale: bool,
    pub stale_subsystems: Vec<String>,
}

/// Compares the knowledge base with the source tree and with Git (M1). A missing index is an
/// error (with a storage-mismatch hint), never reported as drift.
pub fn run_check(project: &ResolvedProject) -> Result<FullDriftReport> {
    let index = project.load_l0_index()?;
    let analysis = detector::analyze_project_with(
        &project.root,
        &detector::AnalyzeOptions {
            deep: false,
            with_rtk: false,
            split_threshold_tokens: project.config.split_threshold_tokens,
            scope: project.config.scope.clone(),
        },
    )?;
    let report = generator::audit(&analysis, &project.knowledge_dir)?;
    let stale_subsystems: Vec<String> = index
        .subsystems
        .iter()
        .filter(|s| project.check_subsystem_staleness(s).0)
        .map(|s| s.id.clone())
        .collect();
    // A subsystem already counted as missing was never in `up_to_date_count`.
    let up_to_date_count = report.up_to_date_count.saturating_sub(
        stale_subsystems
            .iter()
            .filter(|id| !report.missing_in_fs.contains(id))
            .count(),
    );
    let has_drift = !report.missing_in_fs.is_empty()
        || !report.untracked_in_knowledge.is_empty()
        || !stale_subsystems.is_empty();
    Ok(FullDriftReport {
        has_drift,
        missing_in_fs: report.missing_in_fs,
        untracked_in_knowledge: report.untracked_in_knowledge,
        up_to_date_count,
        git_stale: !stale_subsystems.is_empty(),
        stale_subsystems,
    })
}

/// Rejects a `--model` / MCP `model` override naming an alias that is neither `none` nor in
/// `[models]` (M7).
pub fn validate_model_override(config: &Config, model_override: Option<&str>) -> Result<()> {
    let Some(ovr) = model_override else {
        return Ok(());
    };
    // Empty elements are skipped, exactly like `RoleRouter::resolve_chain`.
    for alias in ovr.split(',').map(str::trim).filter(|a| !a.is_empty()) {
        if !(alias.eq_ignore_ascii_case(ROLE_NONE) || config.models.contains_key(alias)) {
            bail!("Unknown model alias '{}' in --model override", alias);
        }
    }
    Ok(())
}

/// Refreshes one Git-stale L1 card without LLM enrichment: re-detects the project, lets
/// `generator::update` regenerate cards it owns, then re-stamps the card's provenance SHAs so
/// that hand-edited cards stop being reported stale too.
pub fn refresh_stale_card(project: &ResolvedProject, sub: &SubsystemInfo) -> Result<()> {
    let analysis = detect(project, false)?;
    let subsystems = build_deterministic(&analysis);
    generator::update(&analysis, &subsystems, &project.knowledge_dir, None)?;
    if let Some(card) = project.card_file(sub) {
        generator::restamp_card_source(&card, &project.root, None)?;
    }
    Ok(())
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

/// Ids of the fresh subsystems whose summary an LLM should (re)write on `update`: new ones,
/// ones whose set of file paths changed, ones already flagged `needs_review`, and ones whose
/// summary is still heuristic. Manual, agent and earlier LLM summaries of unchanged
/// subsystems are left alone (B4).
pub fn subsystems_needing_enrichment(existing: &L0Index, fresh: &[SubsystemInfo]) -> Vec<String> {
    let paths = |s: &SubsystemInfo| -> BTreeSet<String> {
        s.files.iter().map(|f| f.path.clone()).collect()
    };
    fresh
        .iter()
        .filter(
            |f| match existing.subsystems.iter().find(|e| e.id == f.id) {
                None => true,
                Some(e) => {
                    e.needs_review
                        || e.summary_source == SummarySource::Heuristic
                        || paths(e) != paths(f)
                }
            },
        )
        .map(|f| f.id.clone())
        .collect()
}

/// Detects the project and asks the role's model about its subsystems; with `existing` (the
/// `update` case) only about [`subsystems_needing_enrichment`]. The last element is true when
/// `update` had nothing to enrich, so no model was consulted and no package was created.
fn analyze(
    project: &ResolvedProject,
    router: &RoleRouter,
    role: RoleName,
    opts: &PipelineOptions,
    existing: Option<&L0Index>,
) -> Result<(ProjectAnalysis, Vec<SubsystemInfo>, EnrichOutcome, bool)> {
    validate_model_override(&project.config, opts.model_override)?;
    let analysis = detect(project, opts.deep)?;
    let subsystems = build_deterministic(&analysis);
    let prompts = PromptTemplateRegistry::new(project.config.language.clone());
    let targets: Vec<SubsystemInfo> = match existing {
        Some(existing) => {
            let ids = subsystems_needing_enrichment(existing, &subsystems);
            subsystems
                .iter()
                .filter(|s| ids.contains(&s.id))
                .cloned()
                .collect()
        }
        None => subsystems.clone(),
    };
    let nothing_to_enrich = existing.is_some() && targets.is_empty();
    let mut enrich = if nothing_to_enrich {
        EnrichOutcome::Skipped
    } else {
        enrich_subsystems(
            router,
            &prompts,
            role,
            &analysis,
            &targets,
            opts.model_override,
        )
    };
    if let EnrichOutcome::PendingAgent(pkg) = &mut enrich {
        // `context_submit` applies the answer to this project only, re-analyzed at the same
        // depth, and refreshes AGENTS.md when this run wrote it.
        pkg.deep = opts.deep;
        pkg.agents_md = opts.with_agents_md;
        pkg.project_root = Some(project.root.clone());
    }
    Ok((analysis, subsystems, enrich, nothing_to_enrich))
}

/// Deterministic project analysis with the project's scope, split threshold and RTK setting.
pub fn detect(project: &ResolvedProject, deep: bool) -> Result<ProjectAnalysis> {
    detector::analyze_project_with(
        &project.root,
        &detector::AnalyzeOptions {
            deep,
            with_rtk: project.config.integrations.resolve_rtk_active(),
            split_threshold_tokens: project.config.split_threshold_tokens,
            scope: project.config.scope.clone(),
        },
    )
}

/// Applies LLM summaries to the freshly written knowledge base with `llm` provenance. Only
/// subsystems whose summary is still heuristic, or flagged `needs_review` (a non-heuristic
/// summary whose file set changed), are overwritten: manual, agent and earlier LLM summaries
/// stay (B1). Like enrichment itself, a failure here never fails the pipeline: the
/// deterministic base stays and the outcome turns into a warning.
fn apply_enrichment(
    analysis: &ProjectAnalysis,
    knowledge_dir: &Path,
    enrich: EnrichOutcome,
    agents_md: Option<StorageMode>,
) -> EnrichOutcome {
    let EnrichOutcome::Enriched { updates, model } = enrich else {
        return enrich;
    };
    let failed = |e: anyhow::Error| EnrichOutcome::Failed {
        warning: format!("{e:#}"),
    };
    let index = match L0Index::load_from_file(&knowledge_dir.join("L0_index.json")) {
        Ok(index) => index,
        Err(e) => return failed(e),
    };
    let updates: Vec<_> = updates
        .into_iter()
        .filter(|u| {
            index.subsystems.iter().any(|s| {
                s.id == u.id && (s.summary_source == SummarySource::Heuristic || s.needs_review)
            })
        })
        .collect();
    if !updates.is_empty() {
        if let Err(e) = generator::apply_summaries(
            analysis,
            knowledge_dir,
            &updates,
            SummarySource::Llm,
            Some(&model),
            agents_md,
        ) {
            return failed(e);
        }
    }
    EnrichOutcome::Enriched { updates, model }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ci_core::config::{ModelConfig, ModelKind};

    fn cfg_with(alias: &str) -> Config {
        let mut c = Config::default();
        c.models.insert(
            alias.to_string(),
            ModelConfig {
                kind: ModelKind::Agent,
                model: None,
                endpoint: None,
                command: None,
                timeout_ms: None,
                max_input_tokens: None,
            },
        );
        c
    }

    #[test]
    fn model_override_ignores_empty_elements_like_the_router() {
        let c = cfg_with("a");
        assert!(validate_model_override(&c, Some("a,")).is_ok());
        assert!(validate_model_override(&c, Some("a, none")).is_ok());
        assert!(validate_model_override(&c, Some("a,typo")).is_err());
    }
}
