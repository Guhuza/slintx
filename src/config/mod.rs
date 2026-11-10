use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    #[serde(rename = "schema-version", default)]
    pub schema_version: u32,
    #[serde(default)]
    pub template: Option<String>,
    #[serde(rename = "template-version", default)]
    pub template_version: Option<u32>,
    #[serde(rename = "default-package", default)]
    pub default_package: Option<String>,
}

pub fn load(project_root: &Path) -> Result<Option<ProjectConfig>> {
    let path = project_root.join("slintx.toml");
    if !path.is_file() {
        return Ok(None);
    }
    let contents =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let config: ProjectConfig =
        toml::from_str(&contents).with_context(|| format!("failed to parse {}", path.display()))?;
    if config.schema_version != 1 {
        bail!(
            "{} uses unsupported schema-version {}; this SlintX version supports schema 1",
            path.display(),
            config.schema_version
        );
    }
    if let Some(template) = config.template.as_deref()
        && !matches!(template, "basic" | "workspace")
    {
        bail!(
            "{} declares unsupported template `{template}`",
            path.display()
        );
    }
    if config.template_version.is_some_and(|version| version != 1) {
        bail!(
            "{} uses an unsupported template-version; this SlintX version supports version 1",
            path.display()
        );
    }
    Ok(Some(config))
}

pub fn resolve_manifest(manifest: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = manifest {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()?.join(path)
        };
        if !absolute.is_file() {
            bail!("Cargo manifest does not exist: {}", absolute.display());
        }
        return Ok(absolute);
    }

    let mut directory = std::env::current_dir()?;
    loop {
        let candidate = directory.join("Cargo.toml");
        if candidate.is_file() {
            return Ok(candidate);
        }
        if !directory.pop() {
            bail!(
                "no Cargo.toml found; run this command inside a Rust project or pass --manifest-path"
            );
        }
    }
}

pub fn project_root(manifest: &Path) -> Result<&Path> {
    manifest
        .parent()
        .ok_or_else(|| anyhow::anyhow!("manifest path has no parent: {}", manifest.display()))
}

#[cfg(test)]
mod tests {
    use super::load;

    #[test]
    fn rejects_unknown_config_schema_and_fields() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let config_path = temporary.path().join("slintx.toml");
        std::fs::write(&config_path, "schema-version = 2\n").expect("write config");
        assert!(load(temporary.path()).is_err());
        std::fs::write(&config_path, "schema-version = 1\nmystery = true\n").expect("write config");
        assert!(load(temporary.path()).is_err());
    }
}
