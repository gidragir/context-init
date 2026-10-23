//! `pack`: compressed AI context for a subsystem (built-in AST packer or repomix).

use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

use ci_core::analysis::detector::count_tokens;
use ci_core::analysis::pack::{pack_subsystem, PackStyle};
use ci_core::config::is_binary_available;

use super::GlobalArgs;

#[derive(Args, Debug, Clone)]
pub struct PackArgs {
    /// Subsystem id from the L0 index, or `all`
    pub subsystem_id: String,
    /// Project directory
    #[arg(default_value = ".")]
    pub path: PathBuf,
    /// Output file (default: stdout)
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    /// Compress (strip Rust function bodies, normalize whitespace) [default]
    #[arg(long, conflicts_with = "no_compress")]
    pub compress: bool,
    /// Keep full source text
    #[arg(long)]
    pub no_compress: bool,
    /// Use the external `repomix` tool instead of the built-in packer
    #[arg(long)]
    pub repomix: bool,
    /// Output format: xml or markdown
    #[arg(long, default_value = "xml")]
    pub style: String,
}

pub fn run(global: &GlobalArgs, args: &PackArgs) -> Result<ExitCode> {
    let project = global.load_project(&args.path, None)?;
    let index = project.load_l0_index()?;
    let compress = !args.no_compress;
    let packed = pack_subsystem(
        &project.root,
        &project.knowledge_dir,
        &index,
        &args.subsystem_id,
        compress,
        PackStyle::parse(&args.style),
    )?;

    if args.repomix && is_binary_available("repomix") {
        let mut cmd = Command::new("repomix");
        cmd.arg("--include")
            .arg(packed.files.join(","))
            .arg("--style")
            .arg(&args.style)
            .arg("--no-security-check")
            .arg("-o")
            .arg(args.output.as_deref().unwrap_or("-".as_ref()))
            .current_dir(&project.root);
        if compress {
            cmd.arg("--compress");
        }
        if cmd.status().map(|s| s.success()).unwrap_or(false) {
            return Ok(ExitCode::SUCCESS);
        }
        eprintln!("repomix failed; falling back to the built-in packer");
    }

    match &args.output {
        Some(path) => {
            std::fs::write(path, &packed.content)
                .with_context(|| format!("Failed to write {:?}", path))?;
            eprintln!(
                "{}",
                format!(
                    "Packed {} file(s), ~{} tokens into {}",
                    packed.files.len(),
                    count_tokens(&packed.content),
                    path.display()
                )
                .green()
                .bold()
            );
        }
        None => std::io::stdout().write_all(packed.content.as_bytes())?,
    }
    Ok(ExitCode::SUCCESS)
}
