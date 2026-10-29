//! `telemetry report [--submit]` (D30).

use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand};
use std::io::{BufRead, IsTerminal, Write};
use std::process::{Command, ExitCode};

use ci_core::config::is_binary_available;
use ci_core::storage::now_rfc3339;
use ci_core::telemetry::{read_jsonl_file, render_telemetry_report, JsonlTelemetrySink};

use super::GlobalArgs;

const REPO: &str = "gidragir/context-init";
const MAX_URL_BODY_CHARS: usize = 6000;

#[derive(Args, Debug, Clone)]
pub struct TelemetryArgs {
    #[command(subcommand)]
    pub command: TelemetryCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum TelemetryCommand {
    /// Render the aggregated, anonymized digest (Markdown with YAML front matter)
    Report {
        /// Only this month (YYYY-MM); default: all recorded months
        #[arg(long, value_name = "YYYY-MM")]
        month: Option<String>,
        /// Include free-text feedback notes (hidden by default)
        #[arg(long)]
        notes: bool,
        /// Offer the digest as a GitHub issue (via `gh`, or a prefilled link)
        #[arg(long)]
        submit: bool,
        /// Submit without the interactive confirmation
        #[arg(long)]
        yes: bool,
    },
}

/// Prefilled "new issue" link for the digest (body truncated to stay under URL limits).
pub fn issue_url(title: &str, body: &str) -> String {
    let body: String = if body.chars().count() > MAX_URL_BODY_CHARS {
        let cut: String = body.chars().take(MAX_URL_BODY_CHARS).collect();
        format!("{cut}\n\n…(truncated)")
    } else {
        body.to_string()
    };
    format!(
        "https://github.com/{REPO}/issues/new?labels=telemetry&title={}&body={}",
        percent_encode(title),
        percent_encode(&body)
    )
}

fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

pub fn run(global: &GlobalArgs, args: &TelemetryArgs) -> Result<ExitCode> {
    let TelemetryCommand::Report {
        month,
        notes,
        submit,
        yes,
    } = &args.command;
    let project = global.load_project(std::path::Path::new("."), None)?;
    let sink = JsonlTelemetrySink::new(project.storage_paths.data_dir.clone(), true);
    let events = match month {
        Some(m) => {
            let path = sink.file_path_for_month(m);
            if path.is_file() {
                read_jsonl_file(&path)?
            } else {
                Vec::new()
            }
        }
        None => sink.read_all_events()?,
    };
    if events.is_empty() {
        if project.config.telemetry.enabled {
            println!(
                "No telemetry events recorded yet in {}.",
                sink.telemetry_dir().display()
            );
        } else {
            println!(
                "Telemetry is off (opt-in). To record anonymized usage signals, add to your config:\n\n[telemetry]\nenabled = true"
            );
        }
        return Ok(ExitCode::SUCCESS);
    }

    let digest = render_telemetry_report(&events, *notes);
    print!("{digest}");
    if !submit {
        return Ok(ExitCode::SUCCESS);
    }

    if !yes {
        if !std::io::stdin().is_terminal() {
            bail!("refusing to submit without confirmation on a non-interactive stdin; pass --yes");
        }
        eprint!("\nSubmit this digest as a GitHub issue on {REPO}? [y/N] ");
        std::io::stderr().flush()?;
        let mut answer = String::new();
        std::io::stdin().lock().read_line(&mut answer)?;
        if !answer.trim().eq_ignore_ascii_case("y") {
            eprintln!("Not submitted.");
            return Ok(ExitCode::SUCCESS);
        }
    }

    let title = format!(
        "telemetry: {}",
        month
            .clone()
            .unwrap_or_else(|| now_rfc3339()[..7].to_string())
    );
    let gh_ready = is_binary_available("gh")
        && Command::new("gh")
            .args(["auth", "status"])
            .output()
            .is_ok_and(|o| o.status.success());
    if gh_ready {
        let body_file = create_secure_temp_markdown(&digest)?;
        let status = Command::new("gh")
            .args([
                "issue",
                "create",
                "-R",
                REPO,
                "--label",
                "telemetry",
                "--title",
                &title,
                "--body-file",
            ])
            .arg(&body_file)
            .status()
            .context("Failed to run `gh issue create`")?;
        let _ = std::fs::remove_file(&body_file);
        if status.success() {
            return Ok(ExitCode::SUCCESS);
        }
        eprintln!("`gh issue create` failed; open this link instead:");
    }
    println!("\n{}", issue_url(&title, &digest));
    Ok(ExitCode::SUCCESS)
}

/// Writes `content` to a fresh `0o600` file in the temp dir; `create_new` refuses to follow a
/// pre-planted file or symlink at a predictable name.
pub fn create_secure_temp_markdown(content: &str) -> Result<std::path::PathBuf> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let path = std::env::temp_dir().join(format!(
        "context-init-telemetry-{}-{nanos}.md",
        std::process::id()
    ));
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut file = opts
        .open(&path)
        .with_context(|| format!("Failed to create {:?}", path))?;
    file.write_all(content.as_bytes())
        .with_context(|| format!("Failed to write {:?}", path))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue_url_truncates_and_encodes() {
        let url = issue_url("telemetry: 2026-10", "a b&c=д\n");
        assert!(url.starts_with("https://github.com/gidragir/context-init/issues/new?labels=telemetry&title=telemetry%3A%202026-10&body="), "{url}");
        assert!(url.ends_with("a%20b%26c%3D%D0%B4%0A"), "{url}");

        let long = "x".repeat(7000);
        let url = issue_url("t", &long);
        let body = url.split("&body=").nth(1).unwrap();
        assert!(body.starts_with(&"x".repeat(6000)));
        assert!(
            body.ends_with("%E2%80%A6%28truncated%29"),
            "{}",
            &body[body.len() - 40..]
        );
        assert!(!body.contains(&"x".repeat(6001)));
    }

    #[test]
    fn create_secure_temp_markdown_uses_create_new_and_unique_name() {
        let a = create_secure_temp_markdown("digest-a").unwrap();
        let b = create_secure_temp_markdown("digest-b").unwrap();
        assert_ne!(a, b);
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "digest-a");
        assert_eq!(std::fs::read_to_string(&b).unwrap(), "digest-b");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for p in [&a, &b] {
                assert_eq!(
                    std::fs::metadata(p).unwrap().permissions().mode() & 0o777,
                    0o600
                );
            }
        }
        let _ = std::fs::remove_file(a);
        let _ = std::fs::remove_file(b);
    }
}
