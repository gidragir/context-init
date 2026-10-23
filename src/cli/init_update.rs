//! `init` and `update`: the shared detect → enrich → generate pipeline.

use anyhow::Result;
use clap::Args;
use colored::Colorize;
use std::path::PathBuf;
use std::process::ExitCode;

use ci_core::config::RtkMode;
use ci_llm::enrich::cli_outcome_message;
use ci_mcp::pipeline::{self, PipelineOptions};

use super::{output, router_for, GlobalArgs};

/// RTK prefixing flags shared by `init` and `update`.
#[derive(Args, Debug, Clone, Default)]
pub struct RtkArgs {
    /// Always prefix generated commands with `rtk`
    #[arg(long, conflicts_with = "no_rtk")]
    pub with_rtk: bool,
    /// Never prefix generated commands with `rtk`
    #[arg(long)]
    pub no_rtk: bool,
}

impl RtkArgs {
    fn mode(&self) -> Option<RtkMode> {
        match (self.with_rtk, self.no_rtk) {
            (true, _) => Some(RtkMode::On),
            (_, true) => Some(RtkMode::Off),
            _ => None,
        }
    }
}

#[derive(Args, Debug, Clone)]
pub struct InitArgs {
    /// Project directory
    #[arg(default_value = ".")]
    pub path: PathBuf,
    /// Overwrite an existing knowledge base
    #[arg(short, long)]
    pub force: bool,
    /// Analyze and print what would be generated, without writing
    #[arg(long)]
    pub dry_run: bool,
    /// Deep scan: extract function/type signatures
    #[arg(long)]
    pub deep: bool,
    /// Do not create or update the navigation section in the repo's AGENTS.md
    #[arg(long)]
    pub no_agents_md: bool,
    #[command(flatten)]
    pub rtk: RtkArgs,
}

impl Default for InitArgs {
    fn default() -> Self {
        Self {
            path: PathBuf::from("."),
            force: false,
            dry_run: false,
            deep: false,
            no_agents_md: false,
            rtk: RtkArgs::default(),
        }
    }
}

#[derive(Args, Debug, Clone)]
pub struct UpdateArgs {
    /// Project directory
    #[arg(default_value = ".")]
    pub path: PathBuf,
    /// Deep scan: extract function/type signatures
    #[arg(long)]
    pub deep: bool,
    /// Do not update the navigation section in the repo's AGENTS.md
    #[arg(long)]
    pub no_agents_md: bool,
    #[command(flatten)]
    pub rtk: RtkArgs,
}

pub fn run_init(global: &GlobalArgs, args: &InitArgs) -> Result<ExitCode> {
    let project = global.load_project(&args.path, args.rtk.mode())?;
    let router = router_for(&project)?;
    if !global.compact {
        output::banner("Knowledge base init");
    }
    let opts = PipelineOptions {
        deep: args.deep,
        force: args.force,
        dry_run: args.dry_run,
        with_agents_md: !args.no_agents_md,
        model_override: None,
    };
    let out = pipeline::run_init(&project, &router, &opts)?;
    if let Some(msg) = cli_outcome_message(&out.enrich) {
        output::notice(&msg);
    }
    if let Some(notice) = pipeline::carry_over_notice(&out) {
        output::notice(&notice);
    }
    if let Some(hint) = pipeline::legacy_hint(&project, true) {
        output::notice(&hint);
    }

    let verb = if args.dry_run {
        "would initialize"
    } else {
        "initialized"
    };
    if global.compact {
        println!(
            "OK: {verb} {} subsystems in {}",
            out.subsystems.len(),
            out.knowledge_dir.display()
        );
        return Ok(ExitCode::SUCCESS);
    }
    output::analysis_summary(&out.analysis);
    println!();
    for sub in &out.subsystems {
        println!(
            "     - {} ({}) — {}",
            sub.id.yellow(),
            sub.name,
            sub.summary
        );
    }
    println!(
        "\n{} {} subsystems in {}",
        if args.dry_run {
            "DRY RUN:".yellow().bold()
        } else {
            "DONE:".green().bold()
        },
        out.subsystems.len(),
        out.knowledge_dir.display().to_string().cyan()
    );
    Ok(ExitCode::SUCCESS)
}

pub fn run_update(global: &GlobalArgs, args: &UpdateArgs) -> Result<ExitCode> {
    let project = global.load_project(&args.path, args.rtk.mode())?;
    let router = router_for(&project)?;
    if !global.compact {
        output::banner("Knowledge base update");
    }
    let opts = PipelineOptions {
        deep: args.deep,
        with_agents_md: !args.no_agents_md,
        ..PipelineOptions::default()
    };
    let out = pipeline::run_update(&project, &router, &opts)?;
    if let Some(msg) = cli_outcome_message(&out.enrich) {
        output::notice(&msg);
    }
    if global.compact {
        println!(
            "OK: updated {} added, {} refreshed in {}",
            out.added,
            out.updated,
            out.knowledge_dir.display()
        );
    } else {
        println!(
            "{} {} added, {} refreshed in {}",
            "DONE:".green().bold(),
            out.added,
            out.updated,
            out.knowledge_dir.display().to_string().cyan()
        );
    }
    Ok(ExitCode::SUCCESS)
}
