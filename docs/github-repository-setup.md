# GitHub repository health and release checklist

Repository files can prepare automated checks and contribution policy, but they cannot enable
GitHub-hosted security settings or branch rules. This document records the repository's current
verified setup and the remaining crates.io release step.

## Security

- Set repository visibility intentionally and restrict write/admin access to maintainers.
- Enable the dependency graph and Dependabot alerts; review weekly Cargo and Actions update PRs.
- Enable secret scanning and push protection where available.
- Enable private vulnerability reporting and verify that the `SECURITY.md` contact path works.
- Confirm the CodeQL workflow is enabled and its default-branch scan succeeds.

## Branch protection

Protect the default branch (`main`):

- Require pull requests; disallow direct pushes and force pushes for routine contributors.
- Require the `rust` CI matrix to pass before merge.
- Require the `Analyze Rust` CodeQL check when available and successful.
- Require at least one independent approval when the project has another maintainer.
- Dismiss stale approvals on new commits and require conversation resolution.
- Apply restrictions to administrators only if maintainers can still recover a broken branch.

## Repository metadata and release

- `Cargo.toml` and README point to the confirmed canonical repository URL:
  <https://github.com/Guhuza/slintx>.
- Confirm the chosen license and copyright holder before the first release.
- Confirm the `slintx` crates.io package name is available and the publisher has the correct owner
  permissions.
- Review `cargo package --list` and run `cargo publish --dry-run` from a clean checkout.
- Publish only from a reviewed, tagged commit after CI passes; never place a crates.io token in
  the repository.

## Verified repository state

- Public repository: <https://github.com/Guhuza/slintx>, default branch `main`.
- Dependabot alerts and security update pull requests are enabled.
- Secret scanning and push protection are enabled.
- Private vulnerability reporting is enabled and linked from the issue templates and `SECURITY.md`.
- `main` requires pull requests, current CI matrix checks, CodeQL, and conversation resolution;
  force pushes and branch deletion are disabled.
- The CI matrix and CodeQL scan pass on Windows, macOS, and Linux.

The `slintx` package name was available and `cargo publish --dry-run --locked` succeeded. An
authenticated crates.io publishing session is still required to publish the crate.
