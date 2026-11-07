//! `scan`: phase one of the two-phase init (D9) — sizes per directory and extension.

use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use std::process::ExitCode;

use ci_core::analysis::scan::{render_scan_markdown, scan_project_with, ScanOptions};

use super::GlobalArgs;

#[derive(Args, Debug, Clone)]
pub struct ScanArgs {
    /// Project directory
    #[arg(default_value = ".")]
    pub path: PathBuf,
    /// Print the report as JSON
    #[arg(long)]
    pub json: bool,
    /// Directory aggregation depth (1..=5)
    #[arg(long, default_value_t = 2)]
    pub max_depth: usize,
    /// Diagnostic mode: include hidden dot-directories
    #[arg(long)]
    pub include_hidden: bool,
}

pub fn run(global: &GlobalArgs, args: &ScanArgs) -> Result<ExitCode> {
    let project = global.load_project(&args.path, None)?;
    let opts = ScanOptions {
        max_depth: args.max_depth,
        include_hidden: args.include_hidden,
    };
    let report = scan_project_with(&project.root, &project.config.scope, &opts)?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print!(
            "{}",
            render_scan_markdown(&project.project_id, &project.root, &report)
        );
    }
    Ok(ExitCode::SUCCESS)
}
