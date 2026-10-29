//! `scan`: phase one of the two-phase init (D9) — sizes per directory and extension.

use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use std::process::ExitCode;

use ci_core::analysis::scan::{render_scan_markdown, scan_project};

use super::GlobalArgs;

#[derive(Args, Debug, Clone)]
pub struct ScanArgs {
    /// Project directory
    #[arg(default_value = ".")]
    pub path: PathBuf,
    /// Print the report as JSON
    #[arg(long)]
    pub json: bool,
}

pub fn run(global: &GlobalArgs, args: &ScanArgs) -> Result<ExitCode> {
    let project = global.load_project(&args.path, None)?;
    let report = scan_project(&project.root, &project.config.scope)?;
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
