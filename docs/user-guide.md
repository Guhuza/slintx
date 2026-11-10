# SlintX User Guide

## Create a project

```text
slintx new counter
slintx new studio --template workspace --vcs none
```

The output directory must not already exist. Names are normalized to lowercase Cargo package
names; path separators and invalid package-name characters are rejected. `--path` selects the
exact destination directory. Git initialization is enabled by default; use `--vcs none` to
skip it.

Both templates pin `slint` and `slint-build` to 1.18.1 and compile external `.slint` files
through Slint's official build script integration. Each template also defines a `cdylib` library
with Slint's Android activity backend so it can be packaged for mobile platforms.

## Initialize an existing directory

```text
slintx init
slintx init --template workspace
```

`init` writes the selected template into the current directory. It preflights every generated path
and refuses the operation if any output file already exists; existing files are never overwritten.

## Generate a component or page

```text
slintx add component primary-button
slintx add page settings
```

Components are written under `ui/components`; pages are written under `ui/pages`. SlintX prints
the corresponding Slint import statement. It does not modify the root UI or wire page navigation.

## External templates

```text
slintx template list
slintx template add https://example.com/my-slintx-template.git --name my-template
slintx new sample --template my-template
```

A local template directory can be passed instead of a Git URL. The template root must contain
`.slintx-template.toml`:

```toml
schema-version = 1
name = "my-template"
version = "1.0.0"
description = "A custom Slint application"
```

SlintX stores installed templates in the current user's data directory, excludes `.git` and the
metadata file when generating a project, and substitutes `{{project_name}}`, `{{package_name}}`,
and `{{crate_name}}` in text file contents and paths. Symbolic links and special filesystem
entries are rejected. Template content is not executed during installation or generation; Cargo
build scripts and other generated project code should still be reviewed as regular source code.

## Develop

```text
slintx dev
slintx dev --package studio-desktop
slintx dev --bin editor
slintx dev --features my-feature -- --sample-argument
```

SlintX performs a Cargo build, then launches the executable produced by that successful build.
It watches Rust sources, `.slint` files, assets, Cargo manifests/lockfiles, build scripts, and
`slintx.toml`. Repeated changes are coalesced and builds run one at a time.

After a successful rebuild, SlintX stops the prior app instance and starts the new one. On a
failed rebuild it prints the compiler output and does not launch an executable from an older
build. If the previous known-good instance is still running, it remains available until a
successful build replaces it. Press Ctrl+C to stop the watcher and its managed application
process group.

This is rebuild-and-restart development, not runtime hot reload. Slint 1.18.1 has a separate
optional `live-preview` feature for reloading `.slint` UI at runtime; it requires compile-time
feature/environment configuration and is not enabled by SlintX by default.

## Build and check

```text
slintx check
slintx build
slintx build --release --target x86_64-pc-windows-msvc
```

SlintX forwards builds to Cargo and preserves its diagnostics and exit status. Use Cargo's
usual feature, target, package, and profile semantics.

## Diagnose the environment

```text
slintx doctor
slintx doctor --target aarch64-linux-android
```

Doctor reports Rust/Cargo versions and paths, project Slint dependencies, Cargo check status,
installed Rust targets, and platform-specific tools when requested. It never installs tools or
modifies environment variables. `--skip-cargo-check` omits the potentially expensive project
build check.

## Android

Install xbuild with `cargo install xbuild`, then install Android Studio's SDK, NDK, CMake, and
platform-tools. Set `ANDROID_HOME` (or `ANDROID_SDK_ROOT`) and `ANDROID_NDK_ROOT`, install a JDK,
and install the target with `rustup target add aarch64-linux-android`.

```text
slintx doctor --target aarch64-linux-android
slintx android build
slintx android build --arch x64 --format apk
slintx android run --device adb:<device-id>
```

`android run` uses an existing device or emulator ID from `x devices`. Builds package an APK by
default; `--format aab` requests an Android App Bundle. SlintX delegates device deployment and
packaging to xbuild.

## iOS

iOS builds require macOS, Xcode/Command Line Tools, xbuild, and an Apple toolchain. Install the
target with `rustup target add aarch64-apple-ios`, then run:

```text
slintx doctor --target aarch64-apple-ios
slintx ios build
slintx ios build --release
```

xbuild chooses the debug app bundle or release IPA output. The Slint 1.18.1 Rust mobile guide
documents Android; SlintX's iOS command delegates generic Rust iOS packaging to xbuild and has not
been verified on a macOS/Xcode host in this project. Treat it as experimental until built and
launched on an Apple target. Signing identities, provisioning profiles, and App Store submission
remain the developer's responsibility; SlintX does not configure or store Apple credentials.

## Configuration

`slintx.toml` records the template and schema versions and the default package for workspace
development. Unknown or unsupported schema versions are errors; SlintX does not rewrite user
configuration.

Mobile packaging depends on xbuild and native SDKs in addition to Cargo targets. A Rust target by
itself is not sufficient to produce an installable mobile application.
