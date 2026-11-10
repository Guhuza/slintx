# Contributing to SlintX

Thanks for helping improve SlintX. Please open an issue before a substantial change so the
intended behavior and scope can be discussed.

## Development setup

- Install Rust 1.99.0 and Cargo.
- Keep the Slint runtime and build dependencies in generated templates pinned to 1.18.1.
- Do not add platform-specific assumptions to shared command or path handling.

## Before opening a pull request

Run the checks that cover your change:

```text
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo publish --dry-run
```

For template changes, generate and check both Basic and Workspace projects. For mobile changes,
state which host OS, SDK/toolchain, Rust target, and device or simulator were used; do not describe
unverified packaging as supported.

## Pull requests

- Keep changes focused and explain the user-visible behavior.
- Add or update tests for changed behavior.
- Update the README or user guide when command behavior changes.
- Call out platform limitations and breaking changes.
- Do not include credentials, signing keys, or private provisioning data.

By submitting a contribution, you agree that it is licensed under the repository's MIT OR
Apache-2.0 license.
