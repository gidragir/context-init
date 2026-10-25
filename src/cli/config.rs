//! `config init|show|path|validate` (D11).

use anyhow::{bail, Result};
use clap::{Args, Subcommand};
use colored::Colorize;
use std::path::PathBuf;
use std::process::ExitCode;

use ci_core::config::{Config, StoragePaths};

use super::GlobalArgs;

#[derive(Args, Debug, Clone)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ConfigCommand {
    /// Write a documented config template (global by default)
    Init {
        /// Write `<repo>/.context-init.toml` instead of the global config
        #[arg(long)]
        project: bool,
        /// Overwrite an existing file
        #[arg(long)]
        force: bool,
        /// Project directory (for --project)
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
    /// Print the effective (merged) configuration and its source files
    Show {
        /// Print JSON: {"sources": [...], "config": {...}}
        #[arg(long)]
        json: bool,
        /// Project directory
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Print config, storage and telemetry paths for a project
    Path {
        /// Project directory
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Check the configuration cascade; exit 1 listing every problem
    Validate {
        /// Project directory
        #[arg(default_value = ".")]
        path: PathBuf,
    },
}

pub fn run(global: &GlobalArgs, args: &ConfigArgs) -> Result<ExitCode> {
    match &args.command {
        ConfigCommand::Init {
            project,
            force,
            path,
        } => {
            let target = if *project {
                path.join(".context-init.toml")
            } else {
                StoragePaths::resolve(None)
                    .effective_global_config_path()
                    .to_path_buf()
            };
            if target.exists() && !force {
                bail!(
                    "{} already exists; pass --force to overwrite",
                    target.display()
                );
            }
            let written = Config::init_config_file(&target, *force)?;
            println!("Wrote {}", written.display());
            Ok(ExitCode::SUCCESS)
        }
        ConfigCommand::Show { json, path } => {
            let project = global.load_project(path, None)?;
            let sources = project.config.loaded_sources();
            if *json {
                let out = serde_json::json!({ "sources": sources, "config": project.config });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                println!("# sources (lowest priority first; env and CLI flags apply on top):");
                if sources.is_empty() {
                    println!("#   (none: built-in defaults)");
                }
                for src in sources {
                    println!("#   {}", src.display());
                }
                print!("{}", project.config.show_toml()?);
            }
            Ok(ExitCode::SUCCESS)
        }
        ConfigCommand::Path { path } => {
            let project = global.load_project(path, None)?;
            print!(
                "{}",
                project
                    .storage_paths
                    .format_paths(Some(&project.project_id))
            );
            println!("storage: {}", project.config.storage.as_str());
            println!("knowledge_dir: {}", project.knowledge_dir.display());
            Ok(ExitCode::SUCCESS)
        }
        ConfigCommand::Validate { path } => {
            let project = global.load_project(path, None)?;
            let problems = project.config.lint();
            if problems.is_empty() {
                println!(
                    "{} configuration is valid ({} source file(s))",
                    "OK:".green().bold(),
                    project.config.loaded_sources().len()
                );
                return Ok(ExitCode::SUCCESS);
            }
            for p in &problems {
                println!("{} {p}", "error:".red().bold());
            }
            Ok(ExitCode::FAILURE)
        }
    }
}
