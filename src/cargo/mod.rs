use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use command_group::{CommandGroup, GroupChild};
use serde_json::Value;

use crate::cli::CargoArgs;
use crate::config::{self, ProjectConfig};

pub struct BuildArtifact {
    pub executable: PathBuf,
    pub project_root: PathBuf,
}

pub fn invoke_build(args: &CargoArgs, release: bool) -> Result<()> {
    let manifest = config::resolve_manifest(args.manifest_path.as_deref())?;
    let root = config::project_root(&manifest)?;
    let mut command = Command::new("cargo");
    command.arg("build").current_dir(root);
    add_common_args(&mut command, args, &manifest);
    if release {
        command.arg("--release");
    }
    let status = command.status().context("failed to start `cargo build`")?;
    if !status.success() {
        bail!("`cargo build` failed with {status}");
    }
    Ok(())
}

pub fn invoke_check(args: &CargoArgs) -> Result<()> {
    let manifest = config::resolve_manifest(args.manifest_path.as_deref())?;
    let root = config::project_root(&manifest)?;
    let mut command = Command::new("cargo");
    command.arg("check").current_dir(root);
    add_common_args(&mut command, args, &manifest);
    let status = command.status().context("failed to start `cargo check`")?;
    if !status.success() {
        bail!("`cargo check` failed with {status}");
    }
    Ok(())
}

pub fn build_executable(
    args: &CargoArgs,
    release: bool,
    app_package: Option<&str>,
    stopping: &AtomicBool,
) -> Result<Option<BuildArtifact>> {
    let manifest = config::resolve_manifest(args.manifest_path.as_deref())?;
    let root = config::project_root(&manifest)?.to_path_buf();
    let project_config = config::load(&root)?;
    let package = args
        .package
        .as_deref()
        .or_else(|| {
            project_config
                .as_ref()
                .and_then(|cfg| cfg.default_package.as_deref())
        })
        .or(app_package);
    let mut command = Command::new("cargo");
    command
        .arg("build")
        .arg("--message-format=json-render-diagnostics")
        .current_dir(&root);
    add_common_args(&mut command, args, &manifest);
    if args.package.is_none()
        && let Some(package) = package
    {
        command.arg("--package").arg(package);
    }
    if release {
        command.arg("--release");
    }
    command.stdout(std::process::Stdio::piped());
    let mut child = command
        .group_spawn()
        .context("failed to start `cargo build` for dev")?;
    let stdout = match child.inner().stdout.take() {
        Some(stdout) => stdout,
        None => {
            stop_cargo(&mut child).context("failed to clean up Cargo with missing stdout")?;
            bail!("Cargo stdout was not captured");
        }
    };
    let output_reader = match thread::Builder::new()
        .name("slintx-cargo-output".into())
        .spawn({
            let selected_binary = args.bin.clone();
            move || render_cargo_stream(BufReader::new(stdout), selected_binary.as_deref())
        }) {
        Ok(reader) => reader,
        Err(error) => {
            if child
                .try_wait()
                .context("failed to inspect Cargo after output-reader error")?
                .is_none()
            {
                child
                    .kill()
                    .context("failed to stop Cargo after output-reader error")?;
            }
            child
                .wait()
                .context("failed to wait for Cargo after output-reader error")?;
            return Err(error).context("failed to start Cargo output reader");
        }
    };

    let status = match wait_for_cargo(&mut child, stopping) {
        Ok(status) => status,
        Err(error) => {
            let cleanup = stop_cargo(&mut child);
            let reader = output_reader
                .join()
                .map_err(|_| anyhow::anyhow!("Cargo output reader thread panicked"))
                .and_then(|result| result.map(|_| ()));
            if let Err(cleanup_error) = cleanup {
                return Err(error.context(format!(
                    "additionally, failed to clean up Cargo process group: {cleanup_error:#}"
                )));
            }
            if let Err(reader_error) = reader {
                return Err(error.context(format!(
                    "additionally, failed to collect Cargo output: {reader_error:#}"
                )));
            }
            return Err(error);
        }
    };
    let (executable, multiple_executables) = output_reader
        .join()
        .map_err(|_| anyhow::anyhow!("Cargo output reader thread panicked"))??;
    if stopping.load(Ordering::SeqCst) || !status.success() {
        return Ok(None);
    }
    if multiple_executables {
        bail!("Cargo built multiple binary targets; pass --bin <name> to select the application");
    }
    let executable = executable.ok_or_else(|| {
        anyhow::anyhow!(
            "Cargo build succeeded but did not report an executable; select a binary package with --package"
        )
    })?;
    let executable = if executable.is_absolute() {
        executable
    } else {
        root.join(executable)
    };
    if !executable.is_file() {
        bail!(
            "Cargo reported an executable that does not exist: {}",
            executable.display()
        );
    }
    Ok(Some(BuildArtifact {
        executable,
        project_root: root,
    }))
}

fn add_common_args(command: &mut Command, args: &CargoArgs, manifest: &Path) {
    command.arg("--manifest-path").arg(manifest);
    if let Some(package) = &args.package {
        command.arg("--package").arg(package);
    }
    if let Some(target) = &args.target {
        command.arg("--target").arg(target);
    }
    if let Some(features) = &args.features {
        command.arg("--features").arg(features);
    }
    if let Some(binary) = &args.bin {
        command.arg("--bin").arg(binary);
    }
}

fn wait_for_cargo(
    child: &mut GroupChild,
    stopping: &AtomicBool,
) -> Result<std::process::ExitStatus> {
    loop {
        if stopping.load(Ordering::SeqCst) {
            stop_cargo(child).context("failed to stop Cargo process group after Ctrl+C")?;
            return child
                .try_wait()
                .context("failed to collect Cargo exit status")?
                .ok_or_else(|| anyhow::anyhow!("Cargo process did not exit after being stopped"));
        }
        if let Some(status) = child
            .try_wait()
            .context("failed to inspect Cargo build process")?
        {
            return Ok(status);
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn stop_cargo(child: &mut GroupChild) -> Result<()> {
    match child.kill() {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => {}
        Err(error) => return Err(error).context("failed to terminate Cargo process group"),
    }
    child
        .wait()
        .context("failed to wait for Cargo process group to stop")?;
    Ok(())
}

fn render_cargo_stream(
    reader: impl BufRead,
    selected_binary: Option<&str>,
) -> Result<(Option<PathBuf>, bool)> {
    let mut executable = None;
    let mut multiple_executables = false;
    for line in reader.lines() {
        let line = line.context("failed to read Cargo JSON output")?;
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            println!("{line}");
            continue;
        };
        match value.get("reason").and_then(Value::as_str) {
            Some("compiler-message") => {
                if let Some(rendered) = value
                    .get("message")
                    .and_then(|message| message.get("rendered"))
                    .and_then(Value::as_str)
                {
                    print!("{rendered}");
                }
            }
            Some("compiler-artifact") => {
                let is_binary = value
                    .get("target")
                    .and_then(|target| target.get("kind"))
                    .and_then(Value::as_array)
                    .is_some_and(|kinds| kinds.iter().any(|kind| kind.as_str() == Some("bin")));
                let target_binary = value
                    .get("target")
                    .and_then(|target| target.get("name"))
                    .and_then(Value::as_str);
                let matches_selection =
                    selected_binary.is_none_or(|selected| target_binary == Some(selected));
                if is_binary
                    && matches_selection
                    && let Some(path) = value.get("executable").and_then(Value::as_str)
                {
                    if executable.is_some() {
                        multiple_executables = true;
                    } else {
                        executable = Some(PathBuf::from(path));
                    }
                }
            }
            _ => {}
        }
    }
    Ok((executable, multiple_executables))
}

pub fn find_slint_manifest(root_manifest: &Path) -> Result<Option<(PathBuf, ProjectConfig)>> {
    let root = config::project_root(root_manifest)?;
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        let entries = std::fs::read_dir(&directory)
            .with_context(|| format!("failed to read {}", directory.display()))?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                let name = entry.file_name();
                if name != "target"
                    && name != ".git"
                    && name != "templates"
                    && !name.to_string_lossy().starts_with('.')
                {
                    stack.push(path);
                }
                continue;
            }
            if path.file_name().is_some_and(|name| name == "Cargo.toml") {
                let text = std::fs::read_to_string(&path)
                    .with_context(|| format!("failed to read {}", path.display()))?;
                let value: toml::Value = toml::from_str(&text)
                    .with_context(|| format!("failed to parse {}", path.display()))?;
                let deps = value.get("dependencies").and_then(toml::Value::as_table);
                let build_deps = value
                    .get("build-dependencies")
                    .and_then(toml::Value::as_table);
                if deps.is_some_and(|table| table.contains_key("slint"))
                    && build_deps.is_some_and(|table| table.contains_key("slint-build"))
                {
                    let config_root = path.parent().unwrap_or(root);
                    let config = config::load(config_root)?.unwrap_or_default();
                    return Ok(Some((path, config)));
                }
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::render_cargo_stream;
    use std::io::Cursor;

    #[test]
    fn reads_current_binary_artifact_from_cargo_json() {
        let stdout = br#"{"reason":"compiler-artifact","target":{"kind":["bin"]},"executable":"target/debug/app"}"#;
        assert_eq!(
            render_cargo_stream(Cursor::new(stdout), None)
                .expect("valid cargo JSON")
                .0
                .expect("binary artifact")
                .to_string_lossy(),
            "target/debug/app"
        );
    }

    #[test]
    fn selects_only_the_requested_binary_artifact() {
        let output = concat!(
            "{\"reason\":\"compiler-artifact\",\"target\":{\"kind\":[\"bin\"],\"name\":\"first\"},\"executable\":\"target/debug/first\"}\n",
            "{\"reason\":\"compiler-artifact\",\"target\":{\"kind\":[\"bin\"],\"name\":\"second\"},\"executable\":\"target/debug/second\"}\n",
        );
        assert_eq!(
            render_cargo_stream(Cursor::new(output), Some("second"))
                .expect("valid cargo JSON")
                .0
                .expect("selected binary")
                .to_string_lossy(),
            "target/debug/second"
        );
    }

    #[test]
    fn reports_ambiguous_binary_artifacts_after_reading_the_stream() {
        let output = concat!(
            "{\"reason\":\"compiler-artifact\",\"target\":{\"kind\":[\"bin\"],\"name\":\"first\"},\"executable\":\"target/debug/first\"}\n",
            "{\"reason\":\"compiler-artifact\",\"target\":{\"kind\":[\"bin\"],\"name\":\"second\"},\"executable\":\"target/debug/second\"}\n",
        );
        let (executable, multiple) =
            render_cargo_stream(Cursor::new(output), None).expect("valid cargo JSON");
        assert!(executable.is_some());
        assert!(multiple);
    }
}
