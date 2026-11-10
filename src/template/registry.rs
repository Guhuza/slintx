use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use tempfile::Builder;

use super::{read_metadata, validate_registry_name};
use crate::cli::TemplateAddArgs;

pub(super) fn template_registry_dir() -> Result<PathBuf> {
    let data_root = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA")
            .or_else(|| std::env::var_os("APPDATA"))
            .map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join("Library").join("Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .map(|home| home.join(".local").join("share"))
            })
    }
    .ok_or_else(|| {
        anyhow::anyhow!("could not determine the user data directory for SlintX templates")
    })?;
    Ok(data_root.join("SlintX").join("templates"))
}

pub(super) fn add(args: TemplateAddArgs, template_manifest: &str) -> Result<()> {
    let registry = template_registry_dir()?;
    fs::create_dir_all(&registry)
        .with_context(|| format!("failed to create template registry {}", registry.display()))?;
    let staging = Builder::new()
        .prefix(".slintx-template-")
        .tempdir_in(&registry)
        .with_context(|| format!("failed to stage template in {}", registry.display()))?;
    let source_dir = staging.path().join("source");

    let local_source = PathBuf::from(&args.source);
    if let Ok(metadata) = fs::symlink_metadata(&local_source)
        && metadata.file_type().is_symlink()
    {
        bail!(
            "template source must not be a symbolic link: {}",
            local_source.display()
        );
    }
    if local_source.is_dir() {
        copy_directory(&local_source, &source_dir)?;
    } else {
        let status = Command::new("git")
            .args(["clone", "--depth", "1", "--"])
            .arg(&args.source)
            .arg(&source_dir)
            .status()
            .context(
                "failed to start `git clone`; install Git or provide a local template directory",
            )?;
        if !status.success() {
            bail!(
                "`git clone` failed with {status}; template was not installed; verify the repository URL and access"
            );
        }
    }

    let metadata_path = source_dir.join(template_manifest);
    let metadata = read_metadata(&metadata_path)?;
    let name = args.name.as_deref().unwrap_or(&metadata.name);
    validate_registry_name(name)?;
    if matches!(name, "basic" | "workspace") {
        bail!("`{name}` is a built-in template name and cannot be replaced");
    }
    let destination = registry.join(name);
    if destination.exists() {
        bail!(
            "template `{name}` is already installed at {}; choose a different --name",
            destination.display()
        );
    }
    fs::rename(&source_dir, &destination).with_context(|| {
        format!(
            "failed to install template from {} to {}",
            args.source,
            destination.display()
        )
    })?;
    println!(
        "Installed template `{name}` version {} from {}",
        metadata.version, args.source
    );
    Ok(())
}

fn copy_directory(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)
        .with_context(|| format!("failed to create {}", destination.display()))?;
    for entry in fs::read_dir(source)
        .with_context(|| format!("failed to read template source {}", source.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            bail!(
                "template source contains unsupported symlink {}",
                path.display()
            );
        }
        if path.file_name().is_some_and(|name| name == ".git") {
            continue;
        }
        let target = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_directory(&path, &target)?;
        } else if file_type.is_file() {
            fs::copy(&path, &target).with_context(|| {
                format!("failed to copy {} to {}", path.display(), target.display())
            })?;
        } else {
            bail!(
                "template source contains unsupported file type {}",
                path.display()
            );
        }
    }
    Ok(())
}
