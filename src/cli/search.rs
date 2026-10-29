//! `search`: BM25 retrieval over the knowledge base, optionally reranked by the `search` role (D18).

use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use std::process::ExitCode;

use ci_mcp::handlers::DEFAULT_SEARCH_TOP_K;
use ci_mcp::ContextHandler;

use super::{router_for, GlobalArgs};

#[derive(Args, Debug, Clone)]
pub struct SearchArgs {
    /// Search terms or symbols
    pub query: String,
    /// Project directory
    #[arg(default_value = ".")]
    pub path: PathBuf,
    /// Maximum number of results
    #[arg(long, default_value_t = DEFAULT_SEARCH_TOP_K)]
    pub top_k: usize,
}

pub fn run(global: &GlobalArgs, args: &SearchArgs) -> Result<ExitCode> {
    let project = global.load_project(&args.path, None)?;
    let router = router_for(&project)?;
    println!(
        "{}",
        ContextHandler::search_top_k(&project, &args.query, None, &router, args.top_k)?
    );
    Ok(ExitCode::SUCCESS)
}
