//! Transitional legacy wrappers (in-repo `.agents/knowledge`); removed in Task 9.
use anyhow::Result;
pub use ci_core::analysis::generator::*;
use ci_core::analysis::ProjectAnalysis;
use ci_core::storage::{DriftReport, SubsystemInfo};

fn legacy_dir(analysis: &ProjectAnalysis) -> std::path::PathBuf {
    analysis.root_dir.join(".agents").join("knowledge")
}

pub fn generate_knowledge_base(
    analysis: &ProjectAnalysis,
    subsystems: &[SubsystemInfo],
    force: bool,
    with_agents_md: bool,
) -> Result<()> {
    let opts = GenerateOptions {
        force,
        dry_run: false,
        with_agents_md,
    };
    generate(analysis, subsystems, &legacy_dir(analysis), &opts)
}

pub fn audit_knowledge_base(analysis: &ProjectAnalysis) -> Result<DriftReport> {
    let dir = crate::service::ContextService::find_knowledge_dir(&analysis.root_dir)
        .unwrap_or_else(|_| legacy_dir(analysis));
    audit(analysis, &dir)
}

pub fn update_knowledge_base(
    analysis: &ProjectAnalysis,
    new_subsystems: &[SubsystemInfo],
    with_agents_md: bool,
) -> Result<(usize, usize)> {
    update(
        analysis,
        new_subsystems,
        &legacy_dir(analysis),
        with_agents_md,
    )
}
