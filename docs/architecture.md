# Architecture

```text
Clap CLI
  ├─ new/init ── embedded or registered templates ── staged rendering / collision checks
  ├─ template ── local copy or Git clone ── validated per-user template registry
  ├─ add ── Slint UI generators ── no-overwrite file creation
  ├─ dev ── notify ── debounce ── Cargo JSON build ── process-group supervisor
  ├─ build/check ── Cargo subprocess
  ├─ android/ios ── xbuild subprocess ── SDK packaging / device run
  └─ doctor ── tool probes / manifest checks / optional Cargo check

Generated application:
Rust source ── slint::include_modules!()
                  ▲
build.rs ── slint-build 1.18.1 ── .slint UI
```

The first release is one Rust crate organized by responsibility. Cargo owns Rust compilation,
and Slint owns UI compilation and backend selection. SlintX coordinates commands, templates,
watching, process lifecycle, and diagnostics; it does not duplicate either compiler.

The `dev` command is rebuild-and-process-restart. Slint's separately configured `live-preview`
feature is not equivalent to Rust hot reload and is not turned on by default.

Mobile packages are delegated to xbuild rather than reimplementing native packaging. Generated
templates expose a `cdylib` and the Android activity entry point; mobile builds still require the
platform SDKs and are not validated by desktop CI. Slint 1.18.1 documents the Rust Android
workflow; iOS packaging is experimental until verified on a macOS/Xcode host. Apple signing and
store submission are out of scope.

External templates are installed in a per-user data directory. A versioned TOML manifest is
validated before use; Git/local content is copied without running scripts, symlinks are rejected,
and generation is staged before the destination is modified. `init` preflights output collisions
and leaves existing files untouched.

Embedded template manifests are stored as `Cargo.toml.template` in the SlintX source tree and
renamed to `Cargo.toml` when rendered. This prevents Cargo packaging from treating those sample
projects as nested packages and silently excluding them from the published CLI crate.

## Compatibility baseline

- Rust toolchain: 1.99.0
- Slint runtime and build crates: 1.18.1, kept in sync
- Template integration: `build.rs` with `slint_build::compile`, included with
  `slint::include_modules!()`
- Android packaging: Slint's `backend-android-activity-06` feature plus xbuild
- External template manifest: `.slintx-template.toml`, schema 1
