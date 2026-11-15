//! `pack`: compressed AI context for a subsystem (built-in AST packer or repomix).

use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

use ci_core::analysis::detector::count_tokens;
use ci_core::analysis::pack::{pack_subsystem_with, PackMode, PackRequest, PackStyle};
use ci_core::analysis::subsystems::subsystem_tokens;
use ci_core::config::is_binary_available;

use super::{telemetry_scope, GlobalArgs};

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
    /// Output format: compact, xml or markdown
    #[arg(long, default_value = "compact")]
    pub style: String,
    /// Only pack files matching these globs / short file names (repeatable or comma-separated)
    #[arg(long = "files", alias = "filter", value_delimiter = ',')]
    pub files: Vec<String>,
    /// Only pack these symbols (repeatable or comma-separated)
    #[arg(long, value_delimiter = ',')]
    pub symbols: Vec<String>,
    /// Pack mode: full, compressed or toc (overrides --compress/--no-compress)
    #[arg(long)]
    pub mode: Option<String>,
}

/// Maps a built-in pack style to the style name understood by `repomix`.
fn repomix_style(style: PackStyle) -> &'static str {
    match style {
        PackStyle::Compact => "plain",
        PackStyle::Markdown => "markdown",
        PackStyle::Xml => "xml",
    }
}

pub fn run(global: &GlobalArgs, args: &PackArgs) -> Result<ExitCode> {
    let project = global.load_project(&args.path, None)?;
    let index = project.load_l0_index()?;
    let compress = !args.no_compress;
    let style = PackStyle::try_parse(&args.style)?;
    let mode = match &args.mode {
        Some(m) => PackMode::try_parse(m)?,
        None if compress => PackMode::Compressed,
        None => PackMode::Full,
    };
    let mut req = PackRequest::new(&project.root, &args.subsystem_id);
    req.files = &args.files;
    req.symbols = &args.symbols;
    req.mode = mode;
    req.style = style;
    let packed = pack_subsystem_with(&project.root, &project.knowledge_dir, &index, &req)?;

    let tel = telemetry_scope(&project);
    if tel.is_active() {
        let source_tokens: usize = packed
            .subsystems
            .iter()
            .map(|s| subsystem_tokens(&project.root, s))
            .sum();
        tel.efficiency(
            "context_pack",
            source_tokens,
            &packed.content,
            false,
            0,
            false,
            false,
        );
    }

    if args.repomix && is_binary_available("repomix") {
        let mut cmd = Command::new("repomix");
        cmd.arg("--include")
            .arg(packed.files.join(","))
            .arg("--style")
            .arg(repomix_style(style))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repomix_style_maps_all_styles() {
        assert_eq!(repomix_style(PackStyle::Compact), "plain");
        assert_eq!(repomix_style(PackStyle::Markdown), "markdown");
        assert_eq!(repomix_style(PackStyle::Xml), "xml");
    }
}
