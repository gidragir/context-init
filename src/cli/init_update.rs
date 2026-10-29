//! `init` and `update`: the shared detect → enrich → generate pipeline.

use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;
use std::path::PathBuf;
use std::process::ExitCode;

use ci_core::config::{RtkMode, StorageMode};
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
    /// Only look at files matching this glob (repeatable); saved to `[scope]`
    #[arg(long, value_name = "GLOB")]
    pub include: Vec<String>,
    /// Skip files matching this glob (repeatable); saved to `[scope]`
    #[arg(long, value_name = "GLOB")]
    pub exclude: Vec<String>,
    /// Only look at files with this extension (repeatable); saved to `[scope]`
    #[arg(long, value_name = "EXT")]
    pub ext: Vec<String>,
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
            include: Vec::new(),
            exclude: Vec::new(),
            ext: Vec::new(),
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

/// Applies `--include/--exclude/--ext` to the project's scope and persists them as `[scope]` in
/// `<root>/.context-init.toml` (not on `--dry-run`), so `update`, `pack` and `scan` agree.
fn apply_scope_flags(project: &mut ci_mcp::ResolvedProject, args: &InitArgs) -> Result<()> {
    if args.include.is_empty() && args.exclude.is_empty() && args.ext.is_empty() {
        return Ok(());
    }
    let scope = &mut project.config.scope;
    for (target, flags) in [
        (&mut scope.include, &args.include),
        (&mut scope.exclude, &args.exclude),
        (&mut scope.extensions, &args.ext),
    ] {
        if !flags.is_empty() {
            *target = flags.clone();
        }
    }
    ci_core::analysis::scope::validate_scope(&project.config.scope)?;
    if args.dry_run {
        return Ok(());
    }
    let path = project.root.join(".context-init.toml");
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e).with_context(|| format!("Failed to read {path:?}")),
    };
    let mut doc: toml_edit::DocumentMut = text
        .parse()
        .with_context(|| format!("{path:?} is not valid TOML"))?;
    let table = doc
        .entry("scope")
        .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()))
        .as_table_mut()
        .with_context(|| format!("`scope` in {path:?} is not a table"))?;
    for (key, list) in [
        ("include", &project.config.scope.include),
        ("exclude", &project.config.scope.exclude),
        ("extensions", &project.config.scope.extensions),
    ] {
        let mut arr = toml_edit::Array::new();
        for item in list {
            arr.push(item.as_str());
        }
        table[key] = toml_edit::value(arr);
    }
    std::fs::write(&path, doc.to_string()).with_context(|| format!("Failed to write {path:?}"))
}

pub fn run_init(global: &GlobalArgs, args: &InitArgs) -> Result<ExitCode> {
    let mut project = global.load_project(&args.path, args.rtk.mode())?;
    apply_scope_flags(&mut project, args)?;
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
    if let Some(hint) = pipeline::storage_mismatch_hint(&project) {
        output::notice(&hint);
    }
    let out = pipeline::run_init(&project, &router, &opts)?;
    if let Some(msg) = cli_outcome_message(&out.enrich) {
        output::notice(&msg);
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
    if project.storage == StorageMode::Repo {
        println!(
            "Tip: add `.context-init/knowledge/** linguist-generated=true` to .gitattributes to collapse knowledge-base diffs."
        );
    }
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
    if let Some(hint) = pipeline::storage_mismatch_hint(&project) {
        output::notice(&hint);
    }
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
