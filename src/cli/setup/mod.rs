//! `setup <harness>`: user-level MCP registration plus a short rule/skill (D22).

pub mod harness;
pub mod merge;
pub mod rules;

use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;
use std::path::Path;
use std::process::ExitCode;

use ci_core::storage::now_rfc3339;

use harness::{is_installed, server_entry, spec, ConfigFormat, Harness, HomeDirs, RuleTarget};
use merge::{merge_json, merge_toml, PlannedWrite};
use rules::{skill_file, upsert_marked_block, RULE_TEMPLATE};

#[derive(Args, Debug, Clone)]
pub struct SetupArgs {
    /// Harness to register with: antigravity, claude-code, claude-desktop, cursor,
    /// gemini-cli, codex, or `all` (every harness whose config directory exists)
    #[arg(value_parser = parse_target)]
    pub target: SetupTarget,
    /// Apply the changes (default: print the planned diff only)
    #[arg(long)]
    pub write: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum SetupTarget {
    All,
    One(Harness),
}

fn parse_target(s: &str) -> Result<SetupTarget, String> {
    if s.eq_ignore_ascii_case("all") {
        return Ok(SetupTarget::All);
    }
    <Harness as clap::ValueEnum>::from_str(s, true)
        .map(SetupTarget::One)
        .map_err(|_| {
            let names: Vec<&str> = Harness::ALL.iter().map(|h| h.name()).collect();
            format!(
                "unknown harness '{s}'; expected one of: all, {}",
                names.join(", ")
            )
        })
}

/// What `setup` would do for one harness.
pub struct SetupPlan {
    pub harness: Harness,
    pub writes: Vec<PlannedWrite>,
    /// Rule text to show the user when the harness keeps rules outside files.
    pub print_rule: Option<String>,
}

/// Computes all file changes for `h` in memory; nothing is written.
pub fn plan_setup(h: Harness, dirs: &HomeDirs, exe: &Path) -> Result<SetupPlan> {
    let spec = spec(h, dirs);
    let existing = std::fs::read_to_string(&spec.mcp_file).ok();
    let merged = match spec.format {
        ConfigFormat::Json => merge_json(existing.as_deref(), server_entry(h, exe)),
        ConfigFormat::Toml => merge_toml(existing.as_deref(), exe),
    }
    .with_context(|| format!("Cannot update {}", spec.mcp_file.display()))?;
    let mut writes = vec![PlannedWrite::new(spec.mcp_file.clone(), merged)?];
    let mut print_rule = None;
    match spec.rule {
        RuleTarget::SkillFile(path) => writes.push(PlannedWrite::new(path, skill_file())?),
        RuleTarget::MarkedBlock(path) => {
            let current = std::fs::read_to_string(&path).ok();
            let updated = upsert_marked_block(current.as_deref(), RULE_TEMPLATE);
            writes.push(PlannedWrite::new(path, updated)?);
        }
        RuleTarget::PrintOnly => print_rule = Some(RULE_TEMPLATE.to_string()),
        RuleTarget::None => {}
    }
    Ok(SetupPlan {
        harness: h,
        writes,
        print_rule,
    })
}

/// `YYYYMMDD-HHMMSS` (UTC) for backup names.
fn backup_stamp() -> String {
    let ts = now_rfc3339();
    format!(
        "{}{}{}-{}{}{}",
        &ts[0..4],
        &ts[5..7],
        &ts[8..10],
        &ts[11..13],
        &ts[14..16],
        &ts[17..19]
    )
}

pub fn run(args: &SetupArgs) -> Result<ExitCode> {
    let dirs = HomeDirs::from_env()?;
    let exe = std::env::current_exe().context("Cannot locate the context-init executable")?;
    let harnesses: Vec<Harness> = match args.target {
        SetupTarget::One(h) => vec![h],
        SetupTarget::All => Harness::ALL
            .into_iter()
            .filter(|h| is_installed(*h, &dirs))
            .collect(),
    };
    if harnesses.is_empty() {
        println!("No supported harness configuration directories found; nothing to set up.");
        return Ok(ExitCode::SUCCESS);
    }

    // Plan every harness in memory first: an invalid config aborts before any write (M4).
    let plans: Vec<SetupPlan> = harnesses
        .into_iter()
        .map(|h| plan_setup(h, &dirs, &exe))
        .collect::<Result<_>>()?;

    let stamp = backup_stamp();
    for plan in plans {
        println!("{} {}", "==>".cyan().bold(), plan.harness.name().bold());
        for w in &plan.writes {
            if w.is_noop() {
                println!("   unchanged: {}", w.path.display());
                continue;
            }
            if args.write {
                match w.apply(&stamp)? {
                    Some(backup) => println!(
                        "   updated: {} (backup: {})",
                        w.path.display(),
                        backup.display()
                    ),
                    None => println!("   created: {}", w.path.display()),
                }
            } else {
                print!("{}", w.unified_diff());
            }
        }
        if let Some(rule) = &plan.print_rule {
            println!(
                "   Paste into {} → Settings → Rules → User Rules:\n{rule}",
                plan.harness.name()
            );
        }
    }
    if !args.write {
        println!("\nDry run: nothing was written. Re-run with --write to apply (originals are backed up).");
    }
    Ok(ExitCode::SUCCESS)
}
