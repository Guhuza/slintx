use std::ffi::OsString;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "slintx",
    version,
    about = "Project scaffolding and development tools for Rust + Slint"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create a new Slint project from a built-in or installed template.
    New(NewArgs),
    /// Add SlintX project files to the current directory without overwriting files.
    Init(InitArgs),
    /// Add a reusable UI component or page to the current project.
    Add(AddArgs),
    /// Manage installed external templates.
    Template(TemplateArgs),
    /// Build and run an application, then rebuild and restart it on changes.
    Dev(DevArgs),
    /// Build the current project with Cargo.
    Build(BuildArgs),
    /// Check the current project with Cargo.
    Check(CheckArgs),
    /// Diagnose the Rust and Slint development environment.
    Doctor(DoctorArgs),
    /// Build or run an Android application using xbuild.
    Android(AndroidArgs),
    /// Build an iOS application using xbuild (requires macOS).
    Ios(IosArgs),
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum VcsKind {
    Git,
    None,
}

#[derive(Debug, Args)]
pub struct NewArgs {
    pub project_name: String,
    /// Built-in template name or an installed template name from `slintx template list`.
    #[arg(long, default_value = "basic")]
    pub template: String,
    /// Exact output directory. Defaults to a directory named after the project.
    #[arg(long)]
    pub path: Option<PathBuf>,
    #[arg(long, value_enum, default_value = "git")]
    pub vcs: VcsKind,
}

#[derive(Debug, Args)]
pub struct InitArgs {
    #[arg(long, default_value = "basic")]
    pub template: String,
}

#[derive(Debug, Args)]
pub struct AddArgs {
    #[command(subcommand)]
    pub kind: AddKind,
}

#[derive(Debug, Subcommand)]
pub enum AddKind {
    /// Generate a reusable Slint component.
    Component(AddUiArgs),
    /// Generate a standalone Slint page component.
    Page(AddUiArgs),
}

#[derive(Debug, Args)]
pub struct AddUiArgs {
    pub name: String,
}

#[derive(Debug, Args)]
pub struct TemplateArgs {
    #[command(subcommand)]
    pub command: TemplateCommand,
}

#[derive(Debug, Subcommand)]
pub enum TemplateCommand {
    /// List built-in and installed templates.
    List,
    /// Install a local-directory or Git repository template.
    Add(TemplateAddArgs),
}

#[derive(Debug, Args)]
pub struct TemplateAddArgs {
    /// Local template directory or Git clone URL.
    pub source: String,
    /// Registry name used by `slintx new --template`.
    #[arg(long)]
    pub name: Option<String>,
}

#[derive(Debug, Args, Clone, Default)]
pub struct CargoArgs {
    #[arg(long)]
    pub manifest_path: Option<PathBuf>,
    #[arg(long)]
    pub package: Option<String>,
    #[arg(long)]
    pub target: Option<String>,
    #[arg(long)]
    pub features: Option<String>,
    /// Select a binary target when a package contains multiple executables.
    #[arg(long)]
    pub bin: Option<String>,
}

#[derive(Debug, Args, Clone, Default)]
pub struct MobileCargoArgs {
    #[arg(long)]
    pub manifest_path: Option<PathBuf>,
    #[arg(long)]
    pub package: Option<String>,
    #[arg(long)]
    pub features: Option<String>,
}

#[derive(Debug, Args)]
pub struct DevArgs {
    #[command(flatten)]
    pub cargo: CargoArgs,
    #[arg(long)]
    pub release: bool,
    /// Arguments forwarded to the application after `--`.
    #[arg(last = true)]
    pub app_args: Vec<OsString>,
}

#[derive(Debug, Args)]
pub struct BuildArgs {
    #[command(flatten)]
    pub cargo: CargoArgs,
    #[arg(long)]
    pub release: bool,
}

#[derive(Debug, Args)]
pub struct CheckArgs {
    #[command(flatten)]
    pub cargo: CargoArgs,
}

#[derive(Debug, Args)]
pub struct DoctorArgs {
    #[arg(long)]
    pub target: Option<String>,
    /// Skip the project Cargo check.
    #[arg(long)]
    pub skip_cargo_check: bool,
}

#[derive(Debug, Args)]
pub struct AndroidArgs {
    #[command(subcommand)]
    pub command: AndroidCommand,
}

#[derive(Debug, Subcommand)]
pub enum AndroidCommand {
    /// Build an Android APK (requires xbuild and Android tooling).
    Build(AndroidBuildArgs),
    /// Install and run the app on a connected device.
    Run(AndroidRunArgs),
}

#[derive(Debug, Args)]
pub struct AndroidBuildArgs {
    #[command(flatten)]
    pub cargo: MobileCargoArgs,
    #[arg(long, default_value = "arm64")]
    pub arch: String,
    #[arg(long, default_value = "apk")]
    pub format: String,
    #[arg(long)]
    pub release: bool,
}

#[derive(Debug, Args)]
pub struct AndroidRunArgs {
    #[command(flatten)]
    pub cargo: MobileCargoArgs,
    /// Device identifier from `x devices`.
    #[arg(long)]
    pub device: String,
    #[arg(long)]
    pub release: bool,
}

#[derive(Debug, Args)]
pub struct IosArgs {
    #[command(subcommand)]
    pub command: IosCommand,
}

#[derive(Debug, Subcommand)]
pub enum IosCommand {
    /// Build an iOS app bundle or IPA (requires macOS, Xcode, and xbuild).
    Build(IosBuildArgs),
}

#[derive(Debug, Args)]
pub struct IosBuildArgs {
    #[command(flatten)]
    pub cargo: MobileCargoArgs,
    #[arg(long, default_value = "arm64")]
    pub arch: String,
    #[arg(long)]
    pub release: bool,
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{Cli, Command};

    #[test]
    fn parses_new_template_and_vcs_options() {
        let cli = Cli::try_parse_from([
            "slintx",
            "new",
            "Hello_App",
            "--template",
            "workspace",
            "--vcs",
            "none",
        ])
        .expect("valid new command");
        assert!(matches!(cli.command, Some(Command::New(_))));
    }

    #[test]
    fn rejects_unknown_commands() {
        assert!(Cli::try_parse_from(["slintx", "teleport"]).is_err());
    }

    #[test]
    fn parses_mobile_and_project_extension_commands() {
        assert!(Cli::try_parse_from(["slintx", "android", "build"]).is_ok());
        assert!(Cli::try_parse_from(["slintx", "android", "run", "--device", "device-id"]).is_ok());
        assert!(Cli::try_parse_from(["slintx", "ios", "build"]).is_ok());
        assert!(Cli::try_parse_from(["slintx", "init"]).is_ok());
        assert!(Cli::try_parse_from(["slintx", "add", "component", "button"]).is_ok());
        assert!(
            Cli::try_parse_from(["slintx", "template", "add", "https://example.test/t.git"])
                .is_ok()
        );
    }
}
