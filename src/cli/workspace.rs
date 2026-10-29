//! `workspace`: map of the Git repositories inside a directory (D6).

use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use std::process::ExitCode;

use ci_core::config::DEFAULT_MAX_RESPONSE_TOKENS;
use ci_mcp::ContextHandler;

use super::GlobalArgs;

#[derive(Args, Debug, Clone)]
pub struct WorkspaceArgs {
    /// Directory holding the repositories
    #[arg(default_value = ".")]
    pub path: PathBuf,
}

pub fn run(global: &GlobalArgs, args: &WorkspaceArgs) -> Result<ExitCode> {
    let root = std::fs::canonicalize(&args.path)?;
    let budget = global
        .overrides(None)?
        .max_response_tokens
        .unwrap_or(DEFAULT_MAX_RESPONSE_TOKENS);
    println!("{}", ContextHandler::workspace(&root, budget, None)?);
    Ok(ExitCode::SUCCESS)
}
