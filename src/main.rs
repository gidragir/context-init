//! `context-init`: builds and serves a token-efficient L0–L2 knowledge base of a
//! project for AI agents. Thin CLI over the `ci-core`, `ci-llm` and `ci-mcp` crates.

mod cli;

fn main() -> std::process::ExitCode {
    match cli::run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            std::process::ExitCode::FAILURE
        }
    }
}
