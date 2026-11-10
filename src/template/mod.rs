use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use include_dir::{Dir, DirEntry, include_dir};
use serde::Deserialize;
use tempfile::Builder;

use crate::cli::{InitArgs, NewArgs, VcsKind};

mod registry;

static BASIC: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/templates/basic");
static WORKSPACE: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/templates/workspace");
const TEMPLATE_MANIFEST: &str = ".slintx-template.toml";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateMetadata {
    #[serde(rename = "schema-version")]
    pub schema_version: u32,
    pub name: String,
    pub version: String,
    pub description: String,
}

enum TemplateSource {
    Embedded(&'static Dir<'static>),
    Directory(PathBuf),
}

pub fn create(args: NewArgs) -> Result<PathBuf> {
    let package_name = normalize_package_name(&args.project_name)?;
    let source = resolve_template(&args.template)?;
    let current_dir = std::env::current_dir().context("failed to get current directory")?;
    let requested = args.path.unwrap_or_else(|| PathBuf::from(&package_name));
    let target = absolute_path(&current_dir, requested);
    if target.exists() {
        bail!(
            "destination already exists: {}; choose another --path",
            target.display()
        );
    }
    let parent = target
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| anyhow::anyhow!("destination has no parent: {}", target.display()))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create parent directory {}", parent.display()))?;

    if matches!(args.vcs, VcsKind::Git) && !program_available("git") {
        bail!(
            "Git was requested but `git` is not available on PATH; use --vcs none or install Git"
        );
    }

    let staging = Builder::new()
        .prefix(".slintx-new-")
        .tempdir_in(parent)
        .with_context(|| format!("failed to create staging directory in {}", parent.display()))?;
    render_template(&source, staging.path(), &args.project_name, &package_name)?;

    if matches!(args.vcs, VcsKind::Git) {
        let status = Command::new("git")
            .arg("init")
            .current_dir(staging.path())
            .status()
            .context("failed to start `git init`")?;
        if !status.success() {
            bail!(
                "`git init` failed with {status}; the staged project was discarded; use --vcs none or check Git permissions"
            );
        }
    }

    if let Err(error) = fs::rename(staging.path(), &target) {
        return Err(error).with_context(|| {
            format!(
                "failed to move generated project from {} to {}",
                staging.path().display(),
                target.display()
            )
        });
    }
    println!(
        "Created project from template `{}` at {}",
        args.template,
        target.display()
    );
    println!("Next: cd \"{}\" && slintx dev", target.display());
    Ok(target)
}

pub fn init(args: InitArgs) -> Result<()> {
    let source = resolve_template(&args.template)?;
    let root = std::env::current_dir().context("failed to get current directory")?;
    let package_name = normalize_package_name(
        root.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("slint-project"),
    )?;
    let staging = Builder::new()
        .prefix(".slintx-init-")
        .tempdir_in(&root)
        .with_context(|| format!("failed to create staging directory in {}", root.display()))?;
    render_template(&source, staging.path(), &package_name, &package_name)?;

    let files = collect_files(staging.path())?;
    for relative in &files {
        let destination = root.join(relative);
        if destination.exists() {
            bail!(
                "init would overwrite {}; move or rename the existing file and retry",
                destination.display()
            );
        }
        ensure_parent_directories_are_available(&root, &destination)?;
    }

    let mut created_files: Vec<PathBuf> = Vec::new();
    let mut created_directories: Vec<PathBuf> = Vec::new();
    for relative in files {
        let source_path = staging.path().join(&relative);
        let destination = root.join(&relative);
        if let Some(parent) = destination.parent()
            && let Err(error) = create_directories_tracking(parent, &root, &mut created_directories)
        {
            rollback_init(&created_files, &created_directories);
            return Err(error);
        }
        let contents = match fs::read(&source_path) {
            Ok(contents) => contents,
            Err(error) => {
                rollback_init(&created_files, &created_directories);
                return Err(error)
                    .with_context(|| format!("failed to read {}", source_path.display()));
            }
        };
        let mut output = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
        {
            Ok(output) => output,
            Err(error) => {
                rollback_init(&created_files, &created_directories);
                return Err(error).with_context(|| {
                    format!("failed to create project file {}", destination.display())
                });
            }
        };
        created_files.push(destination.clone());
        if let Err(error) = std::io::Write::write_all(&mut output, &contents) {
            rollback_init(&created_files, &created_directories);
            return Err(error).with_context(|| {
                format!("failed to create project file {}", destination.display())
            });
        }
    }
    println!(
        "Initialized project files from template `{}` in {}",
        args.template,
        root.display()
    );
    Ok(())
}

pub fn list() -> Result<()> {
    println!("Built-in templates:");
    println!("  basic      Single-app Rust + Slint project");
    println!("  workspace  Modular Cargo workspace");
    let registry = registry::template_registry_dir()?;
    if !registry.is_dir() {
        return Ok(());
    }
    println!("Installed templates:");
    let mut entries = fs::read_dir(&registry)
        .with_context(|| format!("failed to read template registry {}", registry.display()))?
        .collect::<std::io::Result<Vec<_>>>()
        .with_context(|| format!("failed to list template registry {}", registry.display()))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let manifest = entry.path().join(TEMPLATE_MANIFEST);
        match read_metadata(&manifest) {
            Ok(metadata) => println!(
                "  {}  {} ({})",
                entry.file_name().to_string_lossy(),
                metadata.description,
                metadata.version
            ),
            Err(error) => eprintln!(
                "warning: ignoring invalid installed template {}: {error:#}",
                entry.path().display()
            ),
        }
    }
    Ok(())
}

pub fn add_external_template(args: crate::cli::TemplateAddArgs) -> Result<()> {
    registry::add(args, TEMPLATE_MANIFEST)
}

fn resolve_template(name: &str) -> Result<TemplateSource> {
    match name {
        "basic" => return Ok(TemplateSource::Embedded(&BASIC)),
        "workspace" => return Ok(TemplateSource::Embedded(&WORKSPACE)),
        _ => {}
    }
    let registry = registry::template_registry_dir()?;
    let directory = registry.join(name);
    validate_registry_name(name)?;
    let manifest = directory.join(TEMPLATE_MANIFEST);
    if !directory.is_dir() {
        bail!(
            "template `{name}` is not built in or installed; run `slintx template list` or `slintx template add <source>`"
        );
    }
    read_metadata(&manifest)?;
    Ok(TemplateSource::Directory(directory))
}

fn read_metadata(path: &Path) -> Result<TemplateMetadata> {
    let contents =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let metadata: TemplateMetadata = toml::from_str(&contents)
        .with_context(|| format!("failed to parse template manifest {}", path.display()))?;
    if metadata.schema_version != 1 {
        bail!(
            "{} uses unsupported schema-version {}; expected 1",
            path.display(),
            metadata.schema_version
        );
    }
    validate_registry_name(&metadata.name)
        .with_context(|| format!("{} contains an invalid template name", path.display()))?;
    if metadata.version.trim().is_empty() || metadata.description.trim().is_empty() {
        bail!(
            "{} must declare non-empty version and description fields",
            path.display()
        );
    }
    Ok(metadata)
}

fn render_template(
    source: &TemplateSource,
    destination: &Path,
    project_name: &str,
    package_name: &str,
) -> Result<()> {
    let mut files = Vec::new();
    match source {
        TemplateSource::Embedded(directory) => {
            collect_embedded_files(directory, Path::new(""), &mut files)?;
        }
        TemplateSource::Directory(directory) => {
            collect_directory_files(directory, directory, &mut files)?;
        }
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    for (relative, contents) in files {
        validate_relative_path(&relative)?;
        if relative
            .file_name()
            .is_some_and(|name| name == TEMPLATE_MANIFEST)
        {
            continue;
        }
        let relative = if relative
            .file_name()
            .is_some_and(|name| name == "Cargo.toml.template")
        {
            relative.with_file_name("Cargo.toml")
        } else {
            relative
        };
        let relative = replace_path_tokens(&relative, project_name, package_name)?;
        let contents = match std::str::from_utf8(&contents) {
            Ok(text) => text
                .replace("{{project_name}}", project_name)
                .replace("{{package_name}}", package_name)
                .replace("{{crate_name}}", &package_name.replace('-', "_"))
                .into_bytes(),
            Err(_) => contents,
        };
        let target = destination.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&target, contents)
            .with_context(|| format!("failed to write {}", target.display()))?;
    }
    Ok(())
}

fn collect_embedded_files(
    source: &Dir<'_>,
    prefix: &Path,
    output: &mut Vec<(PathBuf, Vec<u8>)>,
) -> Result<()> {
    for entry in source.entries() {
        match entry {
            DirEntry::Dir(directory) => {
                let name = directory
                    .path()
                    .file_name()
                    .ok_or_else(|| anyhow::anyhow!("template contains an invalid directory"))?;
                let relative = prefix.join(name);
                validate_relative_path(&relative)?;
                collect_embedded_files(directory, &relative, output)?;
            }
            DirEntry::File(file) => {
                let name = file
                    .path()
                    .file_name()
                    .ok_or_else(|| anyhow::anyhow!("template contains an invalid file"))?;
                output.push((prefix.join(name), file.contents().to_vec()));
            }
        }
    }
    Ok(())
}

fn collect_directory_files(
    root: &Path,
    current: &Path,
    output: &mut Vec<(PathBuf, Vec<u8>)>,
) -> Result<()> {
    for entry in fs::read_dir(current)
        .with_context(|| format!("failed to read template directory {}", current.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            bail!("template contains unsupported symlink {}", path.display());
        }
        if path.file_name().is_some_and(|name| name == ".git") {
            continue;
        }
        if file_type.is_dir() {
            collect_directory_files(root, &path, output)?;
        } else if file_type.is_file() {
            let relative = path
                .strip_prefix(root)
                .with_context(|| format!("template path escaped {}", root.display()))?
                .to_path_buf();
            validate_relative_path(&relative)?;
            output.push((
                relative,
                fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?,
            ));
        } else {
            bail!("template contains unsupported file type {}", path.display());
        }
    }
    Ok(())
}

fn collect_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(root)
        .with_context(|| format!("failed to read staging directory {}", root.display()))?
    {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            files.extend(
                collect_files(&entry.path())?
                    .into_iter()
                    .map(|path| PathBuf::from(entry.file_name()).join(path)),
            );
        } else {
            files.push(PathBuf::from(entry.file_name()));
        }
    }
    files.sort();
    Ok(files)
}

fn validate_relative_path(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        bail!("template contains unsafe relative path {}", path.display());
    }
    Ok(())
}

fn replace_path_tokens(path: &Path, project_name: &str, package_name: &str) -> Result<PathBuf> {
    let mut result = PathBuf::new();
    for component in path.components() {
        let Component::Normal(name) = component else {
            bail!("template contains unsafe path {}", path.display());
        };
        let name = name.to_string_lossy();
        let replaced = name
            .replace("{{project_name}}", project_name)
            .replace("{{package_name}}", package_name);
        let replaced_path = Path::new(&replaced);
        validate_relative_path(replaced_path)?;
        result.push(replaced_path);
    }
    validate_relative_path(&result)?;
    Ok(result)
}

fn absolute_path(current_dir: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        current_dir.join(path)
    }
}

fn ensure_parent_directories_are_available(root: &Path, destination: &Path) -> Result<()> {
    let mut current = destination.parent();
    while let Some(directory) = current {
        if directory == root {
            break;
        }
        if directory.exists() && !directory.is_dir() {
            bail!(
                "init needs directory {}, but a file already exists there",
                directory.display()
            );
        }
        current = directory.parent();
    }
    Ok(())
}

fn create_directories_tracking(path: &Path, root: &Path, created: &mut Vec<PathBuf>) -> Result<()> {
    let relative = path
        .strip_prefix(root)
        .with_context(|| format!("generated path {} escaped project root", path.display()))?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            bail!("invalid generated directory {}", path.display());
        };
        current.push(name);
        if !current.exists() {
            match fs::create_dir(&current) {
                Ok(()) => created.push(current.clone()),
                Err(error)
                    if error.kind() == std::io::ErrorKind::AlreadyExists && current.is_dir() => {}
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("failed to create {}", current.display()));
                }
            }
        }
    }
    Ok(())
}

fn rollback_init(files: &[PathBuf], directories: &[PathBuf]) {
    for path in files.iter().rev() {
        if let Err(error) = fs::remove_file(path) {
            eprintln!(
                "failed to roll back generated file {}: {error}",
                path.display()
            );
        }
    }
    for path in directories.iter().rev() {
        if let Err(error) = fs::remove_dir(path) {
            eprintln!(
                "failed to roll back generated directory {}: {error}",
                path.display()
            );
        }
    }
}

pub fn normalize_package_name(name: &str) -> Result<String> {
    let normalized = name.trim().replace('_', "-").to_ascii_lowercase();
    if normalized.is_empty()
        || normalized.len() > 64
        || !normalized.as_bytes()[0].is_ascii_alphanumeric()
        || !normalized
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        || normalized.as_bytes().windows(2).any(|pair| pair == b"--")
    {
        bail!(
            "invalid project name `{name}`; use 1-64 ASCII letters, digits, single hyphens or underscores, starting with a letter or digit"
        );
    }
    Ok(normalized)
}

fn validate_registry_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 64
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        || name.starts_with('-')
        || name.ends_with('-')
        || name.contains("--")
    {
        bail!("invalid template name `{name}`; use lowercase letters, digits, and single hyphens");
    }
    Ok(())
}

fn program_available(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

#[cfg(test)]
mod tests {
    use super::{
        BASIC, TemplateSource, WORKSPACE, collect_files, normalize_package_name, render_template,
        validate_relative_path,
    };
    use std::fs;
    use std::path::Path;

    #[test]
    fn normalizes_cargo_name() {
        assert_eq!(
            normalize_package_name("My_App").expect("valid name"),
            "my-app"
        );
    }

    #[test]
    fn rejects_path_separators_and_invalid_names() {
        assert!(normalize_package_name("../app").is_err());
        assert!(normalize_package_name("-app").is_err());
        assert!(normalize_package_name("你好").is_err());
    }

    #[test]
    fn renders_both_templates_with_valid_relative_paths() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let basic = temporary.path().join("basic");
        fs::create_dir(&basic).expect("create basic output");
        render_template(
            &TemplateSource::Embedded(&BASIC),
            &basic,
            "Sample App",
            "sample-app",
        )
        .expect("render basic");
        assert!(basic.join("src/main.rs").is_file());
        assert!(basic.join("ui/app-window.slint").is_file());
        let manifest = fs::read_to_string(basic.join("Cargo.toml")).expect("read basic manifest");
        assert!(manifest.contains("name = \"sample-app\""));

        let workspace = temporary.path().join("workspace");
        fs::create_dir(&workspace).expect("create workspace output");
        render_template(
            &TemplateSource::Embedded(&WORKSPACE),
            &workspace,
            "Sample App",
            "sample-app",
        )
        .expect("render workspace");
        assert!(workspace.join("apps/desktop/src/main.rs").is_file());
        assert!(workspace.join("crates/app-core/src/lib.rs").is_file());
        let manifest = fs::read_to_string(workspace.join("apps/desktop/Cargo.toml"))
            .expect("read workspace app manifest");
        assert!(manifest.contains("name = \"sample-app-desktop\""));
    }

    #[test]
    fn init_file_collection_uses_relative_paths() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        fs::create_dir_all(temporary.path().join("src")).expect("create source dir");
        fs::write(temporary.path().join("src/main.rs"), "fn main() {}").expect("write source");
        assert!(
            collect_files(temporary.path())
                .expect("collect files")
                .contains(&std::path::PathBuf::from("src/main.rs"))
        );
    }

    #[test]
    fn rejects_unsafe_template_paths() {
        assert!(validate_relative_path(Path::new("../escape")).is_err());
        assert!(validate_relative_path(Path::new("C:\\escape")).is_err());
    }
}
