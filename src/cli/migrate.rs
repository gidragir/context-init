//! `migrate`: move a legacy in-repo `.agents/knowledge` into central storage (D23).

use anyhow::{bail, Result};
use clap::Args;
use colored::Colorize;
use std::path::PathBuf;
use std::process::ExitCode;

use ci_core::storage::{migrate_knowledge, MigrateOptions};

use super::GlobalArgs;

#[derive(Args, Debug, Clone)]
pub struct MigrateArgs {
    /// Project directory
    #[arg(default_value = ".")]
    pub path: PathBuf,
    /// Delete the in-repo copy after a verified migration
    #[arg(long)]
    pub remove_source: bool,
    /// List what would be migrated without writing anything
    #[arg(long)]
    pub dry_run: bool,
    /// Overwrite an existing central knowledge base
    #[arg(long)]
    pub force: bool,
}

pub fn run(global: &GlobalArgs, args: &MigrateArgs) -> Result<ExitCode> {
    let project = global.load_project(&args.path, None)?;
    let target = project
        .storage_paths
        .project_knowledge_dir(&project.project_id);
    if target.join("L0_index.json").is_file() && !args.force && !args.dry_run {
        bail!(
            "central knowledge base already exists at {} (e.g. from `init`), so the in-repo copy is \
             not read anymore. Pass --force only to replace the central one with the in-repo copy.",
            target.display()
        );
    }
    let opts = MigrateOptions {
        remove_source: args.remove_source,
        dry_run: args.dry_run,
    };
    let report = migrate_knowledge(&project.root, &target, &opts)?;

    let head = if args.dry_run {
        "DRY RUN:".yellow().bold()
    } else {
        "MIGRATED:".green().bold()
    };
    if global.compact {
        println!(
            "OK: {} {} files ({} subsystems) {} -> {}",
            if args.dry_run {
                "would migrate"
            } else {
                "migrated"
            },
            report.migrated_files.len(),
            report.subsystems_count,
            report.source_dir.display(),
            report.target_dir.display()
        );
        return Ok(ExitCode::SUCCESS);
    }
    println!(
        "{head} {} -> {} ({} subsystems, schema v{} -> v{})",
        report.source_dir.display(),
        report.target_dir.display().to_string().cyan(),
        report.subsystems_count,
        report.previous_schema_version,
        report.new_schema_version
    );
    for f in &report.migrated_files {
        println!("   • {f}");
    }
    if report.removed_source {
        println!("Removed the in-repo copy.");
    } else if !args.dry_run {
        println!("In-repo copy kept; central storage now takes precedence. Use --remove-source to delete it.");
    }
    Ok(ExitCode::SUCCESS)
}
