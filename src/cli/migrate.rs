//! `migrate`: copy the knowledge base between `repo` and `central` storage.

use anyhow::{bail, Result};
use clap::Args;
use std::path::PathBuf;
use std::process::ExitCode;

use super::GlobalArgs;

#[derive(Args, Debug, Clone)]
pub struct MigrateArgs {
    /// Project directory
    #[arg(default_value = ".")]
    pub path: PathBuf,
}

pub fn run(_global: &GlobalArgs, _args: &MigrateArgs) -> Result<ExitCode> {
    bail!("migrate is being reworked")
}
