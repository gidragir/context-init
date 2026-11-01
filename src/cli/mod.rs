//! Command-line interface: argument parsing, project/config loading and dispatch.

mod check;
mod config;
mod feedback;
mod init_update;
mod mcp;
mod migrate;
mod output;
mod pack;
mod scan;
mod search;
mod setup;
mod task;
mod telemetry;
mod workspace;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use ci_core::config::{CliOverrides, RtkMode};
use ci_core::telemetry::{
    classify_error, harness_from_env, CallContext, CallOutcome, Channel, HarnessId,
    JsonlTelemetrySink, TelemetryScope,
};
use ci_llm::RoleRouter;
use ci_mcp::ResolvedProject;

/// `--version` text: crate version plus the VCS build stamp of `ci-core`.
fn cli_version() -> &'static str {
    static TEXT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    TEXT.get_or_init(|| format!("{} ({})", ci_core::VERSION, ci_core::BUILD))
}

#[derive(Parser, Debug)]
#[command(
    name = "context-init",
    version = cli_version(),
    about = "Builds a token-efficient L0-L2 knowledge base of a project for AI agents (CLI + MCP)"
)]
struct Cli {
    #[command(flatten)]
    global: GlobalArgs,

    #[command(subcommand)]
    command: Option<Commands>,
}

/// Flags accepted by every command (D11, D16).
#[derive(Args, Debug, Clone, Default)]
pub struct GlobalArgs {
    /// Model alias from `[models]` used for every role in this run
    #[arg(long, global = true, value_name = "ALIAS")]
    pub model: Option<String>,

    /// Per-role model chain override, repeatable: init|update|task|search=<alias[,alias...]>
    #[arg(long = "role", global = true, value_name = "ROLE=ALIAS")]
    pub roles: Vec<String>,

    /// Extra config file, applied above project and global config
    #[arg(long, global = true, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// One-line machine-readable output (no banners or colors)
    #[arg(long, global = true)]
    pub compact: bool,
}

impl GlobalArgs {
    /// Converts the flags into configuration-cascade overrides.
    pub fn overrides(&self, rtk: Option<RtkMode>) -> Result<CliOverrides> {
        let mut cli = CliOverrides {
            model: self.model.clone(),
            rtk,
            ..CliOverrides::default()
        };
        for spec in &self.roles {
            let (role, chain) = CliOverrides::parse_role_override(spec)?;
            cli.roles.insert(role.as_str().to_string(), chain);
        }
        Ok(cli)
    }

    /// Loads the project at `path` with the full configuration cascade.
    pub fn load_project(&self, path: &Path, rtk: Option<RtkMode>) -> Result<ResolvedProject> {
        let project = ResolvedProject::load_with(
            path.to_path_buf(),
            &self.overrides(rtk)?,
            self.config.as_deref(),
        )?;
        ci_mcp::pipeline::validate_model_override(&project.config, self.model.as_deref())?;
        Ok(project)
    }
}

/// Builds the role router for a loaded project.
pub fn router_for(project: &ResolvedProject) -> Result<RoleRouter> {
    Ok(RoleRouter::new(project.config.clone(), None)?.with_project_id(project.project_id.clone()))
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Build the knowledge base from scratch
    Init(init_update::InitArgs),
    /// Re-detect the project and merge changes into the knowledge base
    Update(init_update::UpdateArgs),
    /// Check the knowledge base for drift against the source tree
    Check(check::CheckArgs),
    /// Survey the tree: files, lines and tokens per directory and extension
    Scan(scan::ScanArgs),
    /// Digest of the changes since a base ref (L1 overview, L2 with --target)
    Task(task::TaskArgs),
    /// Map the Git repositories inside a directory
    Workspace(workspace::WorkspaceArgs),
    /// Search the knowledge base (BM25, optional LLM rerank)
    Search(search::SearchArgs),
    /// Pack a subsystem's files into compressed AI context
    Pack(pack::PackArgs),
    /// Run the MCP server on stdio
    Mcp(mcp::McpArgs),
    /// Inspect, create and validate configuration
    Config(config::ConfigArgs),
    /// Copy the knowledge base between repo and central storage
    Migrate(migrate::MigrateArgs),
    /// Register the MCP server (and a short agent rule) with an AI harness
    Setup(setup::SetupArgs),
    /// Opt-in usage telemetry: digest and submission
    Telemetry(telemetry::TelemetryArgs),
    /// Rate a context-init tool (recorded only when telemetry is enabled)
    Feedback(feedback::FeedbackArgs),
}

/// Telemetry tool name of a command; `mcp` (long-lived server) and `telemetry` are not instrumented.
fn command_tool_name(cmd: &Commands) -> Option<&'static str> {
    match cmd {
        Commands::Init(_) => Some("init"),
        Commands::Update(_) => Some("update"),
        Commands::Check(_) => Some("check"),
        Commands::Scan(_) => Some("scan"),
        Commands::Task(_) => Some("task"),
        Commands::Workspace(_) => Some("workspace"),
        Commands::Search(_) => Some("search"),
        Commands::Pack(_) => Some("pack"),
        Commands::Config(_) => Some("config"),
        Commands::Migrate(_) => Some("migrate"),
        Commands::Setup(_) => Some("setup"),
        Commands::Feedback(_) => Some("feedback"),
        Commands::Mcp(_) | Commands::Telemetry(_) => None,
    }
}

/// Project directory a command operates on (current directory when it takes none).
fn command_path(cmd: &Commands) -> PathBuf {
    match cmd {
        Commands::Init(a) => a.path.clone(),
        Commands::Update(a) => a.path.clone(),
        Commands::Check(a) => a.path.clone(),
        Commands::Scan(a) => a.path.clone(),
        Commands::Task(a) => a.path.clone(),
        Commands::Workspace(a) => a.path.clone(),
        Commands::Search(a) => a.path.clone(),
        Commands::Pack(a) => a.path.clone(),
        Commands::Migrate(a) => a.path.clone(),
        Commands::Feedback(a) => a.path.clone(),
        Commands::Mcp(_) | Commands::Config(_) | Commands::Setup(_) | Commands::Telemetry(_) => {
            PathBuf::from(".")
        }
    }
}

/// Per-call telemetry context of this process: CLI channel, harness from environment markers.
fn call_context() -> CallContext {
    let harness = harness_from_env(|k| std::env::var_os(k).is_some());
    CallContext {
        harness,
        channel: Channel::Cli,
        nested: harness != HarnessId::Terminal,
    }
}

/// Telemetry scope for `project`: writes only when the project's own config enables telemetry.
pub fn telemetry_scope(project: &ResolvedProject) -> TelemetryScope {
    let sink = JsonlTelemetrySink::new(project.storage_paths.data_dir.clone(), true);
    TelemetryScope {
        recorder: project.recorder(Some(Arc::new(sink))),
        ctx: call_context(),
    }
}

/// Records the operational event of a finished command; a non-zero `ExitCode` without an error is `Ok`.
/// Failures to load the project or write the event are ignored.
fn record_call(
    global: &GlobalArgs,
    path: &Path,
    tool: &'static str,
    started: Instant,
    result: &Result<ExitCode>,
) {
    let Ok(project) = global.load_project(path, None) else {
        return;
    };
    let outcome = match result {
        Ok(_) => CallOutcome::Ok,
        Err(e) => CallOutcome::Err(classify_error(e)),
    };
    let scope = telemetry_scope(&project);
    scope
        .recorder
        .record_call(&scope.ctx, tool, started, outcome);
}

/// Parses arguments and runs the selected command, recording one operational telemetry event.
pub fn run() -> Result<ExitCode> {
    let cli = Cli::parse();
    if cli.global.compact {
        colored::control::set_override(false);
    }
    let Cli { global, command } = cli;
    let started = Instant::now();
    let instrumented = match &command {
        Some(cmd) => command_tool_name(cmd).map(|tool| (tool, command_path(cmd))),
        None => Some(("init", PathBuf::from("."))),
    };
    let result = dispatch(&global, command);
    if let Some((tool, path)) = instrumented {
        record_call(&global, &path, tool, started, &result);
    }
    result
}

fn dispatch(global: &GlobalArgs, command: Option<Commands>) -> Result<ExitCode> {
    match command {
        None => init_update::run_init(global, &init_update::InitArgs::default()),
        Some(Commands::Init(args)) => init_update::run_init(global, &args),
        Some(Commands::Update(args)) => init_update::run_update(global, &args),
        Some(Commands::Check(args)) => check::run(global, &args),
        Some(Commands::Scan(args)) => scan::run(global, &args),
        Some(Commands::Task(args)) => task::run(global, &args),
        Some(Commands::Workspace(args)) => workspace::run(global, &args),
        Some(Commands::Search(args)) => search::run(global, &args),
        Some(Commands::Pack(args)) => pack::run(global, &args),
        Some(Commands::Mcp(args)) => mcp::run(&args),
        Some(Commands::Config(args)) => config::run(global, &args),
        Some(Commands::Migrate(args)) => migrate::run(global, &args),
        Some(Commands::Setup(args)) => setup::run(&args),
        Some(Commands::Telemetry(args)) => telemetry::run(global, &args),
        Some(Commands::Feedback(args)) => feedback::run(global, &args),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(std::iter::once("context-init").chain(args.iter().copied())).unwrap()
    }

    #[test]
    fn check_parsing_defaults_and_flags() {
        match parse(&["check"]).command {
            Some(Commands::Check(a)) => {
                assert_eq!(a.path, PathBuf::from("."));
                assert!(!a.exit_code);
            }
            other => panic!("expected check, got {other:?}"),
        }
        let cli = parse(&["check", "--exit-code", "--compact", "/custom/path"]);
        assert!(cli.global.compact);
        match cli.command {
            Some(Commands::Check(a)) => {
                assert_eq!(a.path, PathBuf::from("/custom/path"));
                assert!(a.exit_code);
            }
            other => panic!("expected check, got {other:?}"),
        }
    }

    #[test]
    fn pack_parsing_compress_by_default() {
        match parse(&["pack", "core"]).command {
            Some(Commands::Pack(a)) => {
                assert_eq!(a.subsystem_id, "core");
                assert!(!a.no_compress);
                assert_eq!(a.style, "xml");
                assert!(a.output.is_none());
            }
            other => panic!("expected pack, got {other:?}"),
        }
        assert!(Cli::try_parse_from([
            "context-init",
            "pack",
            "core",
            "--compress",
            "--no-compress"
        ])
        .is_err());
    }

    #[test]
    fn global_flags_after_subcommand_and_role_parsing() {
        let cli = parse(&[
            "init",
            ".",
            "--model",
            "local",
            "--role",
            "init=a,b",
            "--role",
            "update=none",
        ]);
        assert_eq!(cli.global.model.as_deref(), Some("local"));
        let ov = cli.global.overrides(None).unwrap();
        assert_eq!(ov.roles["init"], vec!["a".to_string(), "b".to_string()]);
        assert_eq!(ov.roles["update"], vec!["none".to_string()]);
        assert!(parse(&["--role", "bogus=x"])
            .global
            .overrides(None)
            .is_err());
    }
}
