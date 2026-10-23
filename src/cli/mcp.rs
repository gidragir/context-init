//! `mcp`: the global stdio MCP server (D21).

use anyhow::{Context, Result};
use clap::Args;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Args, Debug, Clone)]
pub struct McpArgs {
    /// Fallback project root when a tool call names none and the client sends no roots
    #[arg(default_value = ".")]
    pub path: PathBuf,
}

pub fn run(args: &McpArgs) -> Result<ExitCode> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("Failed to start the Tokio runtime")?;
    rt.block_on(ci_mcp::run_mcp_server(Some(&args.path)))?;
    Ok(ExitCode::SUCCESS)
}
