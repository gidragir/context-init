//! Human-readable output helpers.

use ci_core::analysis::ProjectAnalysis;
use colored::Colorize;

pub fn banner(subtitle: &str) {
    let rule = "======================================================="
        .cyan()
        .bold();
    println!("{rule}");
    println!(
        "{} {}",
        "CONTEXT-INIT:".cyan().bold(),
        subtitle.yellow().bold()
    );
    println!("{rule}");
}

pub fn analysis_summary(analysis: &ProjectAnalysis) {
    println!("   • Project:      {}", analysis.name.yellow().bold());
    println!(
        "   • Stack:        {}",
        analysis.project_type.display_name().magenta()
    );
    if !analysis.description.is_empty() {
        println!("   • Description:  {}", analysis.description);
    }
    println!(
        "   • Modules:      {}",
        analysis.detected_modules.len().to_string().blue()
    );
    if !analysis.workspace_members.is_empty() {
        println!(
            "   • Workspace:    {}",
            analysis.workspace_members.join(", ").blue()
        );
    }
    if !analysis.root_entrypoints.is_empty() {
        println!(
            "   • Entrypoints:  {}",
            analysis.root_entrypoints.join(", ").cyan()
        );
    }
    if let Some(cmd) = &analysis.commands.test {
        println!("   • Test command: {}", cmd.green());
    }
    if let Some(cmd) = &analysis.commands.build {
        println!("   • Build command:{}", format!(" {cmd}").green());
    }
}

/// Prints a notice line to stderr (keeps stdout machine-readable).
pub fn notice(msg: &str) {
    eprintln!("{} {msg}", "note:".yellow().bold());
}
