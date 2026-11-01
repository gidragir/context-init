//! Exposes the VCS build stamp as `CONTEXT_INIT_BUILD` (`<short sha>[-dirty]`, `unknown` on any error).

use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn main() {
    let build = match git(&["rev-parse", "--short", "HEAD"]).filter(|s| !s.is_empty()) {
        Some(sha) => match git(&["status", "--porcelain"]) {
            Some(status) if !status.is_empty() => format!("{sha}-dirty"),
            _ => sha,
        },
        None => "unknown".to_string(),
    };
    println!("cargo:rustc-env=CONTEXT_INIT_BUILD={build}");
    if let Some(head) = git(&["rev-parse", "--git-path", "HEAD"]) {
        let p = std::path::Path::new(&head);
        if p.exists() {
            println!("cargo:rerun-if-changed={head}");
        }
    }
    if let Some(idx) = git(&["rev-parse", "--git-path", "index"]) {
        let p = std::path::Path::new(&idx);
        if p.exists() {
            println!("cargo:rerun-if-changed={idx}");
        }
    }
}
