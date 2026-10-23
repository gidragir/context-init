//! Command-line interface: argument parsing, project/config loading and dispatch.

mod check;
mod init_update;
mod mcp;
mod output;
mod pack;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ci_core::config::{CliOverrides, RtkMode};
use ci_llm::RoleRouter;
use ci_mcp::ResolvedProject;

#[derive(Parser, Debug)]
#[command(
    name = "context-init",
    version,
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
        ResolvedProject::load_with(
            path.to_path_buf(),
            &self.overrides(rtk)?,
            self.config.as_deref(),
        )
    }
}

/// Builds the role router for a loaded project.
pub fn router_for(project: &ResolvedProject) -> Result<RoleRouter> {
    Ok(RoleRouter::new(project.config.clone(), None)?.with_project_id(project.project_id.clone()))
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Build the knowledge base from scratch (into central storage)
    Init(init_update::InitArgs),
    /// Re-detect the project and merge changes into the knowledge base
    Update(init_update::UpdateArgs),
    /// Check the knowledge base for drift against the source tree
    Check(check::CheckArgs),
    /// Pack a subsystem's files into compressed AI context
    Pack(pack::PackArgs),
    /// Run the MCP server on stdio
    Mcp(mcp::McpArgs),
}

/// Parses arguments and runs the selected command.
pub fn run() -> Result<ExitCode> {
    let cli = Cli::parse();
    if cli.global.compact {
        colored::control::set_override(false);
    }
    match cli.command {
        None => init_update::run_init(&cli.global, &init_update::InitArgs::default()),
        Some(Commands::Init(args)) => init_update::run_init(&cli.global, &args),
        Some(Commands::Update(args)) => init_update::run_update(&cli.global, &args),
        Some(Commands::Check(args)) => check::run(&cli.global, &args),
        Some(Commands::Pack(args)) => pack::run(&cli.global, &args),
        Some(Commands::Mcp(args)) => mcp::run(&args),
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
