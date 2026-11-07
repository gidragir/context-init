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
    /// Explicit repositories to inspect (repeatable)
    #[arg(long = "repo")]
    pub repos: Vec<PathBuf>,
    /// Continue from the `next_cursor` of the previous page
    #[arg(long)]
    pub cursor: Option<String>,
}

pub fn run(global: &GlobalArgs, args: &WorkspaceArgs) -> Result<ExitCode> {
    let root = if args.path.exists() {
        std::fs::canonicalize(&args.path)?
    } else {
        args.path.clone()
    };
    let budget = global
        .overrides(None)?
        .max_response_tokens
        .unwrap_or(DEFAULT_MAX_RESPONSE_TOKENS);
    let explicit_paths = if args.repos.is_empty() {
        None
    } else {
        Some(args.repos.as_slice())
    };
    println!(
        "{}",
        ContextHandler::workspace(&root, explicit_paths, budget, args.cursor.as_deref())?
    );
    Ok(ExitCode::SUCCESS)
}
