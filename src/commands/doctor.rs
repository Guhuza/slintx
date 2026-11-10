use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use tracing::info;

use crate::cargo;
use crate::cli::DoctorArgs;
use crate::config;

#[derive(Default)]
struct Report {
    failures: usize,
}

impl Report {
    fn pass(&mut self, label: &str, detail: impl AsRef<str>) {
        println!("[ok]   {label}: {}", detail.as_ref());
    }

    fn warn(&mut self, label: &str, detail: impl AsRef<str>) {
        println!("[skip] {label}: {}", detail.as_ref());
    }

    fn fail(&mut self, label: &str, detail: impl AsRef<str>) {
        self.failures += 1;
        println!("[fail] {label}: {}", detail.as_ref());
    }
}

pub fn run(args: DoctorArgs) -> Result<()> {
    let mut report = Report::default();
    let rustc = command_version("rustc", &["--version"]);
    match rustc {
        Some(version) => {
            if version_at_least(&version, (1, 99, 0)) {
                report.pass("Rust", version);
            } else {
                report.fail(
                    "Rust",
                    format!("{version}; install Rust 1.99.0 or newer with rustup"),
                );
            }
        }
        None => report.fail("Rust", "rustc not found on PATH; install Rust using rustup"),
    }

    match command_version("cargo", &["--version"]) {
        Some(version) => report.pass("Cargo", version),
        None => report.fail(
            "Cargo",
            "cargo not found on PATH; install Rust using rustup",
        ),
    }

    let target = args.target.clone().or_else(host_target);
    if let Some(target) = target.as_deref() {
        check_target(&mut report, target);
        if target.contains("android") {
            check_android(&mut report);
        }
        if target.contains("android") || target.contains("ios") {
            check_mobile_packager(&mut report);
        }
        check_host_build_tools(&mut report, target);
    }

    match config::resolve_manifest(None) {
        Ok(manifest) => {
            check_slint_project(&mut report, &manifest)?;
            if args.skip_cargo_check {
                report.warn("Cargo check", "skipped by --skip-cargo-check");
            } else {
                match cargo::invoke_check(&crate::cli::CargoArgs {
                    manifest_path: Some(manifest),
                    target: args.target.clone(),
                    ..Default::default()
                }) {
                    Ok(()) => report.pass("Cargo check", "completed successfully"),
                    Err(error) => report.fail("Cargo check", format!("{error:#}")),
                }
            }
        }
        Err(error) if error.to_string().starts_with("no Cargo.toml found") => report.warn(
            "Project",
            "no Cargo.toml found; run doctor inside a project for Slint and Cargo checks",
        ),
        Err(error) => return Err(error),
    }

    fn check_mobile_packager(report: &mut Report) {
        check_program(
            report,
            "x",
            &["--version"],
            "install xbuild with `cargo install xbuild`",
        );
    }

    match command_version("git", &["--version"]) {
        Some(version) => report.pass("Git", version),
        None => report.warn(
            "Git",
            "not found on PATH; only required for `slintx new --vcs git`",
        ),
    }

    if report.failures > 0 {
        anyhow::bail!("{} doctor check(s) failed", report.failures);
    }
    info!("doctor checks completed");
    Ok(())
}

fn check_slint_project(report: &mut Report, root_manifest: &Path) -> Result<()> {
    match cargo::find_slint_manifest(root_manifest)? {
        Some((manifest, _config)) => {
            let contents = std::fs::read_to_string(&manifest)
                .with_context(|| format!("failed to read {}", manifest.display()))?;
            let value: toml::Value = toml::from_str(&contents)
                .with_context(|| format!("failed to parse {}", manifest.display()))?;
            let workspace = workspace_manifest(&manifest)?;
            let runtime = dependency_version(&value, workspace.as_ref(), "dependencies", "slint");
            let build = dependency_version(
                &value,
                workspace.as_ref(),
                "build-dependencies",
                "slint-build",
            );
            match (runtime.as_deref(), build.as_deref()) {
                (Some("=1.18.1"), Some("=1.18.1")) => report.pass(
                    "Slint dependencies",
                    format!(
                        "slint and slint-build are both pinned to 1.18.1 ({})",
                        manifest.display()
                    ),
                ),
                (Some(runtime), Some(build)) => report.fail(
                    "Slint dependencies",
                    format!("found slint {runtime} and slint-build {build}; keep both at =1.18.1"),
                ),
                _ => report.fail(
                    "Slint dependencies",
                    format!(
                        "slint and slint-build dependencies must both be declared in {}",
                        manifest.display()
                    ),
                ),
            }
        }
        None => report.warn(
            "Slint project",
            "no Cargo package declares both slint and slint-build",
        ),
    }
    Ok(())
}

fn workspace_manifest(manifest: &Path) -> Result<Option<toml::Value>> {
    let start = manifest.parent().unwrap_or_else(|| Path::new("."));
    for directory in start.ancestors() {
        let candidate = directory.join("Cargo.toml");
        if !candidate.is_file() {
            continue;
        }
        let contents = std::fs::read_to_string(&candidate)
            .with_context(|| format!("failed to read {}", candidate.display()))?;
        let value: toml::Value = toml::from_str(&contents)
            .with_context(|| format!("failed to parse {}", candidate.display()))?;
        if value.get("workspace").is_some() {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

fn dependency_version(
    value: &toml::Value,
    workspace: Option<&toml::Value>,
    section: &str,
    dependency_name: &str,
) -> Option<String> {
    let dependency = value.get(section)?.get(dependency_name)?;
    let version = dependency
        .as_str()
        .map(str::to_owned)
        .or_else(|| dependency.get("version")?.as_str().map(str::to_owned));
    if version.is_some() {
        return version;
    }
    if !dependency
        .get("workspace")
        .and_then(toml::Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }
    let workspace_dependency = workspace?
        .get("workspace")?
        .get("dependencies")?
        .get(dependency_name)?;
    workspace_dependency
        .as_str()
        .map(str::to_owned)
        .or_else(|| {
            workspace_dependency
                .get("version")?
                .as_str()
                .map(str::to_owned)
        })
}

fn check_target(report: &mut Report, target: &str) {
    match Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
    {
        Ok(output) if output.status.success() => {
            let installed = String::from_utf8_lossy(&output.stdout);
            if installed.lines().any(|line| line.trim() == target) {
                report.pass("Rust target", target);
            } else {
                report.fail(
                    "Rust target",
                    format!("{target} is not installed; run `rustup target add {target}`"),
                );
            }
        }
        Ok(output) => report.fail(
            "Rust target",
            format!(
                "rustup target query failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        ),
        Err(error) => report.fail(
            "Rust target",
            format!("could not run rustup ({error}); install rustup and target {target}"),
        ),
    }
}

fn host_target() -> Option<String> {
    let output = Command::new("rustc").arg("-vV").output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("host: ").map(str::to_owned))
}

fn check_host_build_tools(report: &mut Report, target: &str) {
    if target.contains("windows-msvc") {
        match Command::new("where.exe").arg("cl.exe").output() {
            Ok(output) if output.status.success() => report.pass(
                "MSVC compiler",
                String::from_utf8_lossy(&output.stdout).lines().next().unwrap_or("cl.exe found"),
            ),
            _ => report.warn(
                "MSVC compiler",
                "cl.exe is not visible on PATH; use a Visual Studio Developer Command Prompt when native dependencies require it",
            ),
        }
    } else if target.contains("apple") {
        match Command::new("xcode-select").arg("-p").output() {
            Ok(output) if output.status.success() => report.pass(
                "Xcode command line tools",
                String::from_utf8_lossy(&output.stdout).trim(),
            ),
            _ => report.warn(
                "Xcode command line tools",
                "not detected; install Xcode Command Line Tools if native dependencies require them",
            ),
        }
    } else if target.contains("linux") {
        check_optional_program(report, "C compiler", "cc", &["--version"]);
        check_optional_program(report, "pkg-config", "pkg-config", &["--version"]);
    }
    check_optional_program(report, "CMake", "cmake", &["--version"]);
}

fn check_optional_program(report: &mut Report, label: &str, program: &str, args: &[&str]) {
    match command_version(program, args) {
        Some(version) => report.pass(label, version.lines().next().unwrap_or("available")),
        None => report.warn(
            label,
            format!("{program} not found on PATH; install it if a platform dependency requires it"),
        ),
    }
}

fn check_android(report: &mut Report) {
    let sdk = env_path("ANDROID_HOME").or_else(|| env_path("ANDROID_SDK_ROOT"));
    check_directory(
        report,
        "Android SDK",
        sdk.as_deref(),
        "set ANDROID_HOME or ANDROID_SDK_ROOT to the SDK directory",
    );
    let ndk = env_path("ANDROID_NDK_ROOT");
    check_directory(
        report,
        "Android NDK",
        ndk.as_deref(),
        "set ANDROID_NDK_ROOT to an installed NDK directory",
    );
    check_program(
        report,
        "adb",
        &["--version"],
        "install Android platform-tools",
    );
    check_javac(report);
    if let Some(ndk) = ndk.as_deref() {
        let properties = ndk.join("source.properties");
        match std::fs::read_to_string(&properties) {
            Ok(contents) => {
                if let Some(revision) = contents
                    .lines()
                    .find_map(|line| line.strip_prefix("Pkg.Revision = "))
                {
                    report.pass("Android NDK revision", revision);
                } else {
                    report.warn(
                        "Android NDK revision",
                        format!("could not parse {}", properties.display()),
                    );
                }
            }
            Err(error) => report.warn(
                "Android NDK revision",
                format!("could not read {}: {error}", properties.display()),
            ),
        }
    }
    check_program(
        report,
        "cmake",
        &["--version"],
        "install CMake from Android SDK Tools if required by the packaging workflow",
    );
}

fn check_javac(report: &mut Report) {
    if let Some(version) = command_version("javac", &["-version"]) {
        report.pass("Java compiler", version);
        return;
    }

    if let Some(home) = env_path("JAVA_HOME") {
        let javac = home
            .join("bin")
            .join(format!("javac{}", std::env::consts::EXE_SUFFIX));
        if let Some(version) = command_version_path(&javac, &["-version"]) {
            report.pass(
                "Java compiler",
                format!("{} ({})", version.trim(), javac.display()),
            );
            return;
        }
        report.fail(
            "Java compiler",
            format!("JAVA_HOME is set but {} could not be run", javac.display()),
        );
        return;
    }

    report.fail(
        "Java compiler",
        "javac not found; install a JDK and add its bin directory to PATH or set JAVA_HOME",
    );
}

fn env_path(name: &str) -> Option<std::path::PathBuf> {
    std::env::var_os(name).map(std::path::PathBuf::from)
}

fn check_directory(report: &mut Report, label: &str, path: Option<&Path>, suggestion: &str) {
    match path {
        Some(path) if path.is_dir() => report.pass(label, path.display().to_string()),
        Some(path) => report.fail(
            label,
            format!("{} is not a directory; {suggestion}", path.display()),
        ),
        None => report.fail(label, suggestion),
    }
}

fn check_program(report: &mut Report, program: &str, args: &[&str], suggestion: &str) {
    match command_version(program, args) {
        Some(version) => report.pass(program, version),
        None => report.fail(program, format!("not available; {suggestion}")),
    }
}

fn command_version(program: &str, args: &[&str]) -> Option<String> {
    command_version_path(Path::new(program), args)
}

fn command_version_path(program: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = if output.stdout.is_empty() {
        &output.stderr
    } else {
        &output.stdout
    };
    Some(String::from_utf8_lossy(text).trim().to_owned())
}

fn version_at_least(output: &str, minimum: (u64, u64, u64)) -> bool {
    let Some(version) = output
        .split_whitespace()
        .find(|part| part.as_bytes().first().is_some_and(u8::is_ascii_digit))
    else {
        return false;
    };
    let mut parts = version.split('.').filter_map(|part| {
        part.split(|character: char| !character.is_ascii_digit())
            .next()
            .and_then(|digits| digits.parse::<u64>().ok())
    });
    let current = (parts.next(), parts.next(), parts.next());
    match current {
        (Some(major), Some(minor), Some(patch)) => (major, minor, patch) >= minimum,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::version_at_least;

    #[test]
    fn compares_rust_versions() {
        assert!(version_at_least("rustc 1.99.0 (hash date)", (1, 99, 0)));
        assert!(version_at_least("rustc 1.99.1 (hash date)", (1, 99, 0)));
        assert!(!version_at_least("rustc 1.98.9 (hash date)", (1, 99, 0)));
        assert!(!version_at_least("version unknown", (1, 99, 0)));
    }
}
