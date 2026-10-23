//! `feedback <tool> <verdict>`: explicit feedback on a context-init tool (D29).

use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use std::process::ExitCode;

use ci_core::telemetry::{record_feedback, FeedbackSignal};

use super::GlobalArgs;

#[derive(Args, Debug, Clone)]
pub struct FeedbackArgs {
    /// Tool or command being rated (e.g. context_pack, check)
    pub tool: String,
    /// Verdict: helpful, unhelpful, accurate, stale, ...
    pub verdict: String,
    /// Optional free-text note (hidden from reports unless --notes)
    #[arg(long)]
    pub note: Option<String>,
    /// Project directory
    #[arg(long, default_value = ".")]
    pub path: PathBuf,
}

pub fn run(global: &GlobalArgs, args: &FeedbackArgs) -> Result<ExitCode> {
    let project = global.load_project(&args.path, None)?;
    let signal = FeedbackSignal {
        tool: args.tool.clone(),
        verdict: args.verdict.clone(),
        note: args.note.clone(),
    };
    if record_feedback(
        &project.config,
        &project.storage_paths,
        &project.project_id,
        signal,
    )? {
        println!("Feedback recorded: {} = {}", args.tool, args.verdict);
    } else {
        println!(
            "Feedback not recorded: telemetry is off. Enable it with `[telemetry] enabled = true` in your config."
        );
    }
    Ok(ExitCode::SUCCESS)
}
