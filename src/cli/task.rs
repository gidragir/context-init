//! `task`: digest of the changes since a base ref (D4, D5).

use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use std::process::ExitCode;

use ci_mcp::ContextHandler;

use super::GlobalArgs;

#[derive(Args, Debug, Clone)]
pub struct TaskArgs {
    /// Project directory
    #[arg(default_value = ".")]
    pub path: PathBuf,
    /// Compare against this ref (merge-base with HEAD; default HEAD~1)
    #[arg(long, value_name = "REF")]
    pub base: Option<String>,
    /// Files the task is about (repeatable)
    #[arg(long = "paths", value_name = "PATH", num_args = 1..)]
    pub paths: Vec<String>,
    /// Drill down into one method or metadata object (L2)
    #[arg(long, value_name = "SYMBOL_OR_OBJECT")]
    pub target: Option<String>,
}

pub fn run(global: &GlobalArgs, args: &TaskArgs) -> Result<ExitCode> {
    let project = global.load_project(&args.path, None)?;
    let out = ContextHandler::task(
        &project,
        Some(&args.paths),
        args.base.as_deref(),
        args.target.as_deref(),
        None,
    )?;
    println!("{out}");
    Ok(ExitCode::SUCCESS)
}
