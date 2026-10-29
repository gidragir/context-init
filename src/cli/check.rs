//! `check`: drift between the knowledge base and the source tree.

use anyhow::Result;
use clap::Args;
use colored::Colorize;
use std::path::PathBuf;
use std::process::ExitCode;

use ci_mcp::pipeline;

use super::{output, GlobalArgs};

#[derive(Args, Debug, Clone)]
pub struct CheckArgs {
    /// Project directory
    #[arg(default_value = ".")]
    pub path: PathBuf,
    /// Exit with code 1 when drift is detected
    #[arg(long)]
    pub exit_code: bool,
}

pub fn run(global: &GlobalArgs, args: &CheckArgs) -> Result<ExitCode> {
    let project = global.load_project(&args.path, None)?;
    let report = pipeline::run_check(&project)?;
    let has_drift = report.has_drift;

    if global.compact {
        if has_drift {
            println!(
                "DRIFT: up_to_date={}, missing=[{}], untracked=[{}], git_stale={}",
                report.up_to_date_count,
                report.missing_in_fs.join(", "),
                report.untracked_in_knowledge.join(", "),
                report.git_stale
            );
        } else {
            println!("OK: {} subsystems up-to-date", report.up_to_date_count);
        }
    } else {
        output::banner("Drift check");
        println!(
            "   • Up-to-date subsystems: {}",
            report.up_to_date_count.to_string().green().bold()
        );
        if !report.missing_in_fs.is_empty() {
            println!("   ! Gone from disk, still in the knowledge base:");
            for m in &report.missing_in_fs {
                println!("      - {}", m.red());
            }
        }
        if !report.untracked_in_knowledge.is_empty() {
            println!("   ! New modules missing from the knowledge base:");
            for u in &report.untracked_in_knowledge {
                println!("      - {}", u.yellow());
            }
            println!("\nRun `context-init update` to sync them.");
        }
        if !report.stale_subsystems.is_empty() {
            println!("   ! Cards older than their sources in Git:");
            for st in &report.stale_subsystems {
                println!("      - {}", st.yellow());
            }
            println!("\nRun `context-init update` to refresh them.");
        }
        if !has_drift {
            println!(
                "{}",
                "Knowledge base is in sync with the source tree."
                    .green()
                    .bold()
            );
        }
    }
    Ok(if has_drift && args.exit_code {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}
