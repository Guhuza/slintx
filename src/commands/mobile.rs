use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::cli::{
    AndroidArgs, AndroidBuildArgs, AndroidCommand, AndroidRunArgs, IosArgs, IosBuildArgs,
    IosCommand, MobileCargoArgs,
};
use crate::config;

pub fn android(args: AndroidArgs) -> Result<()> {
    match args.command {
        AndroidCommand::Build(args) => android_build(args),
        AndroidCommand::Run(args) => android_run(args),
    }
}

pub fn ios(args: IosArgs) -> Result<()> {
    match args.command {
        IosCommand::Build(args) => ios_build(args),
    }
}

fn android_build(args: AndroidBuildArgs) -> Result<()> {
    validate_arch(&args.arch)?;
    if !matches!(args.format.as_str(), "apk" | "aab") {
        bail!(
            "unsupported Android format `{}`; use `apk` or `aab`",
            args.format
        );
    }
    let mut command = xbuild_command("build", &args.cargo)?;
    command
        .args(["--platform", "android", "--arch"])
        .arg(&args.arch)
        .args(["--format", &args.format]);
    if args.release {
        command.arg("--release");
    }
    run_xbuild(command)
}

fn android_run(args: AndroidRunArgs) -> Result<()> {
    let mut command = xbuild_command("run", &args.cargo)?;
    command.args(["--device", &args.device]);
    if args.release {
        command.arg("--release");
    }
    run_xbuild(command)
}

fn ios_build(args: IosBuildArgs) -> Result<()> {
    if !cfg!(target_os = "macos") {
        bail!("iOS builds require macOS with Xcode; this SlintX host is not macOS");
    }
    validate_arch(&args.arch)?;
    let mut command = xbuild_command("build", &args.cargo)?;
    command
        .args(["--platform", "ios", "--arch"])
        .arg(&args.arch);
    if args.release {
        command.arg("--release");
    }
    run_xbuild(command)
}

fn xbuild_command(subcommand: &str, cargo: &MobileCargoArgs) -> Result<Command> {
    let manifest = config::resolve_manifest(cargo.manifest_path.as_deref())?;
    let root = config::project_root(&manifest)?.to_path_buf();
    let project_config = crate::config::load(&root)?;
    let mut command = Command::new("x");
    command
        .arg(subcommand)
        .arg("--manifest-path")
        .arg(manifest)
        .current_dir(root);
    let package = cargo.package.as_deref().or_else(|| {
        project_config
            .as_ref()
            .and_then(|config| config.default_package.as_deref())
    });
    if let Some(package) = package {
        command.args(["--package", package]);
    }
    if let Some(features) = &cargo.features {
        command.args(["--features", features]);
    }
    Ok(command)
}

fn run_xbuild(mut command: Command) -> Result<()> {
    let status = command
        .status()
        .context("failed to start xbuild (`x`); install it with `cargo install xbuild`")?;
    if !status.success() {
        bail!("xbuild command failed with {status}");
    }
    Ok(())
}

fn validate_arch(arch: &str) -> Result<()> {
    if !matches!(arch, "arm64" | "x64") {
        bail!("unsupported xbuild architecture `{arch}`; use `arm64` or `x64`");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_arch;

    #[test]
    fn limits_mobile_architectures_to_xbuild_supported_values() {
        assert!(validate_arch("arm64").is_ok());
        assert!(validate_arch("x64").is_ok());
        assert!(validate_arch("armv7").is_err());
    }
}
