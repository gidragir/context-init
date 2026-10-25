//! `migrate`: copy the knowledge base between `repo` and `central` storage.

use anyhow::{bail, Context, Result};
use clap::Args;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;

use ci_core::config::StorageMode;
use ci_core::storage::{copy_knowledge, CopyKbOptions};

use super::{output, GlobalArgs};

#[derive(Args, Debug, Clone)]
pub struct MigrateArgs {
    /// Project directory
    #[arg(default_value = ".")]
    pub path: PathBuf,
    /// Storage mode to move the knowledge base to: repo or central
    #[arg(long, value_parser = parse_mode)]
    pub to: StorageMode,
    /// Delete the source copy after a verified migration
    #[arg(long)]
    pub remove_source: bool,
    /// List what would be copied without writing anything
    #[arg(long)]
    pub dry_run: bool,
    /// Overwrite an existing knowledge base in the target location
    #[arg(long)]
    pub force: bool,
}

fn parse_mode(raw: &str) -> Result<StorageMode, String> {
    StorageMode::from_str(raw).map_err(|e| e.to_string())
}

fn other(mode: StorageMode) -> StorageMode {
    match mode {
        StorageMode::Repo => StorageMode::Central,
        StorageMode::Central => StorageMode::Repo,
    }
}

/// Records `storage = "<mode>"` in `<root>/.context-init.toml`, creating the file if
/// needed and keeping every other key and comment.
fn set_storage_key(root: &Path, mode: StorageMode) -> Result<PathBuf> {
    let path = root.join(".context-init.toml");
    let text = if path.is_file() {
        fs::read_to_string(&path).with_context(|| format!("Failed to read {:?}", path))?
    } else {
        String::new()
    };
    let mut doc: toml_edit::DocumentMut = text
        .parse()
        .with_context(|| format!("Failed to parse {:?}", path))?;
    doc["storage"] = toml_edit::value(mode.as_str());
    fs::write(&path, doc.to_string()).with_context(|| format!("Failed to write {:?}", path))?;
    Ok(path)
}

pub fn run(global: &GlobalArgs, args: &MigrateArgs) -> Result<ExitCode> {
    let project = global.load_project(&args.path, None)?;
    let to = args.to;
    let source = project.knowledge_dir_for(other(to));
    let target = project.knowledge_dir_for(to);
    let target_has_index = target.join("L0_index.json").is_file();

    if project.storage == to && target_has_index {
        println!("already using storage = \"{}\"", to.as_str());
        return Ok(ExitCode::SUCCESS);
    }
    if target_has_index && !args.force && !args.dry_run {
        bail!(
            "a knowledge base already exists at {}; pass --force to replace it with the copy from {}",
            target.display(),
            source.display()
        );
    }

    let report = copy_knowledge(
        &source,
        &target,
        &CopyKbOptions {
            dry_run: args.dry_run,
            remove_source: args.remove_source,
        },
    )?;
    if !args.dry_run {
        set_storage_key(&project.root, to)?;
        if report.removed_source && other(to) == StorageMode::Repo {
            // `.context-init/` is left empty once the repo copy is gone.
            let _ = fs::remove_dir(project.root.join(".context-init"));
        }
    }

    let verb = if args.dry_run {
        "would migrate"
    } else {
        "migrated"
    };
    if global.compact {
        println!(
            "OK: {verb} {} files ({} subsystems) {} -> {}; storage = \"{}\"",
            report.files.len(),
            report.subsystems_count,
            source.display(),
            target.display(),
            to.as_str()
        );
        return Ok(ExitCode::SUCCESS);
    }
    output::notice(&format!(
        "{verb} {} -> {} ({} subsystems)",
        source.display(),
        target.display(),
        report.subsystems_count
    ));
    for f in &report.files {
        println!("   • {f}");
    }
    if report.removed_source {
        println!("Removed the source copy.");
    } else if !args.dry_run {
        println!("Source copy kept; pass --remove-source to delete it.");
    }
    Ok(ExitCode::SUCCESS)
}
