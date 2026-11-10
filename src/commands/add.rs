use std::fs;

use anyhow::{Context, Result, bail};

use crate::cargo;
use crate::cli::{AddArgs, AddKind};
use crate::config;

pub fn run(args: AddArgs) -> Result<()> {
    let manifest = config::resolve_manifest(None)?;
    let (package_manifest, _) = cargo::find_slint_manifest(&manifest)?.ok_or_else(|| {
        anyhow::anyhow!("no package with slint and slint-build dependencies was found")
    })?;
    let package_root = package_manifest
        .parent()
        .ok_or_else(|| anyhow::anyhow!("package manifest has no parent directory"))?;
    let ui_root = package_root.join("ui");
    if !ui_root.is_dir() {
        bail!(
            "Slint UI directory {} does not exist; create it or use a SlintX project template",
            ui_root.display()
        );
    }

    let (kind, name) = match args.kind {
        AddKind::Component(args) => ("component", args.name),
        AddKind::Page(args) => ("page", args.name),
    };
    let component_name = component_name(&name)?;
    let file_name = file_name(&name)?;
    let (directory, contents) = match kind {
        "component" => (
            ui_root.join("components"),
            format!("export component {component_name} inherits Rectangle {{\n}}\n"),
        ),
        _ => (
            ui_root.join("pages"),
            format!(
                "export component {component_name} inherits Window {{\n    width: 640px;\n    height: 480px;\n\n    Text {{\n        text: \"{component_name}\";\n    }}\n}}\n"
            ),
        ),
    };
    fs::create_dir_all(&directory)
        .with_context(|| format!("failed to create {}", directory.display()))?;
    let destination = directory.join(format!("{file_name}.slint"));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)
        .with_context(|| {
            format!(
                "cannot create {}; it may already exist",
                destination.display()
            )
        })?;
    use std::io::Write;
    file.write_all(contents.as_bytes())
        .with_context(|| format!("failed to write {}", destination.display()))?;
    println!(
        "Created Slint {kind} {component_name} at {}",
        destination.display()
    );
    println!(
        "Import it from your UI with: import {{ {component_name} }} from \"{kind}s/{file_name}.slint\";"
    );
    Ok(())
}

fn component_name(name: &str) -> Result<String> {
    let mut result = String::new();
    for segment in name.split(['-', '_']) {
        if segment.is_empty() || !segment.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
            bail!("invalid UI name `{name}`; use ASCII letters, digits, hyphens, or underscores");
        }
        let mut chars = segment.chars();
        let first = chars
            .next()
            .ok_or_else(|| anyhow::anyhow!("invalid empty UI name segment"))?;
        if result.is_empty() && !first.is_ascii_alphabetic() {
            bail!("UI names must start with an ASCII letter");
        }
        result.push(first.to_ascii_uppercase());
        result.extend(chars);
    }
    if result.is_empty() {
        bail!("UI name must not be empty");
    }
    Ok(result)
}

fn file_name(name: &str) -> Result<String> {
    let normalized = name.trim().replace('_', "-").to_ascii_lowercase();
    if normalized.is_empty()
        || !normalized
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        || normalized.starts_with('-')
        || normalized.ends_with('-')
        || normalized.contains("--")
    {
        bail!("invalid UI name `{name}`; use ASCII letters, digits, hyphens, or underscores");
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::{component_name, file_name};

    #[test]
    fn normalizes_ui_names_to_identifiers_and_paths() {
        assert_eq!(
            component_name("hello-world").expect("valid name"),
            "HelloWorld"
        );
        assert_eq!(file_name("Hello_World").expect("valid name"), "hello-world");
    }

    #[test]
    fn rejects_names_that_cannot_form_slint_identifiers() {
        assert!(component_name("../escape").is_err());
        assert!(component_name("1-page").is_err());
        assert!(component_name("").is_err());
    }
}
