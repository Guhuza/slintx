# SlintX

SlintX is a Rust CLI for creating and developing Rust + Slint applications.
It delegates UI compilation to Slint, Rust builds to Cargo, and mobile packaging to xbuild.

## Requirements

- Rust 1.99.0 (the repository toolchain is pinned in `rust-toolchain.toml`)
- Cargo

Slint templates use Slint 1.18.1 and `slint-build` 1.18.1.
SlintX is licensed under either [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.

## Commands

```text
slintx new <project-name> [--template <name>] [--path <directory>] [--vcs git|none]
slintx init [--template <name>]
slintx add component <name>
slintx add page <name>
slintx template list
slintx template add <local-directory|git-url> [--name <name>]
slintx dev [--manifest-path <Cargo.toml>] [--package <name>] [--bin <name>] [--features <features>] [-- <app args>]
slintx build [--release] [--manifest-path <Cargo.toml>] [--package <name>] [--target <triple>]
slintx check [--manifest-path <Cargo.toml>] [--package <name>] [--target <triple>]
slintx doctor [--target <triple>] [--skip-cargo-check]
slintx android build [--arch arm64|x64] [--format apk|aab] [--release]
slintx android run --device <id> [--release]
slintx ios build [--arch arm64|x64] [--release]
slintx --help
slintx --version
```

`android` and `ios` use the separately installed `x` executable from
[xbuild](https://github.com/rust-mobile/xbuild). `doctor --target aarch64-linux-android`
checks Android requirements; iOS builds require macOS and Xcode.

`dev` watches project source, Slint UI, assets, and Cargo inputs. It rebuilds with Cargo and
restarts the application only after a successful build. A failed build never starts an old
artifact. This is process restart, not Rust hot reload. Slint's optional `live-preview` feature
is separate and is not enabled by default.

## Quick start

```text
slintx new hello-slint
cd hello-slint
slintx dev
```

The `workspace` template contains `apps/desktop` and a small `crates/app-core` library. Both
templates include a `cdylib` library entry point for mobile packaging and keep Slint runtime/build
crates pinned to 1.18.1. Their Cargo manifests are stored as `Cargo.toml.template` in this
repository and restored to `Cargo.toml` by the generator so they are retained in the published
SlintX package.

`init` adds template files to the current directory and refuses to replace existing files.
`add component` and `add page` create importable Slint files under `ui/components` and `ui/pages`;
they do not edit the existing application UI automatically.

External templates can be installed from a local directory or a Git URL. They must contain a
root `.slintx-template.toml` with `schema-version = 1`, `name`, `version`, and `description`.
Templates are copied without executing template scripts; text files support `{{project_name}}`,
`{{package_name}}`, and `{{crate_name}}` substitutions.

## Development

```text
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

See [the user guide](docs/user-guide.md), [troubleshooting guide](docs/troubleshooting.md),
[contributing guide](CONTRIBUTING.md), and
[GitHub repository setup checklist](docs/github-repository-setup.md).

Source code and issue tracking: <https://github.com/Guhuza/slintx>.

## Mobile scope

The mobile commands delegate target builds, APK/AAB or iOS bundle/IPA packaging, and device
installation to xbuild. They do not configure Apple signing identities, provisioning profiles, or
store publishing. The Android workflow follows Slint 1.18.1's documented activity backend; the
iOS path is experimental pending verification on macOS/Xcode. See
[the user guide](docs/user-guide.md) for platform prerequisites.
