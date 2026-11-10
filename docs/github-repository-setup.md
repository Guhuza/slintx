# GitHub repository setup checklist

Repository files can prepare automated checks and contribution policy, but they cannot enable
GitHub-hosted security settings or branch rules. Complete these steps after the repository URL
exists and the repository is connected as the local `origin`.

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

- Add the confirmed canonical repository URL to `Cargo.toml` (`repository` and, if desired,
  `homepage`) and README links.
- Confirm the chosen license and copyright holder before the first release.
- Confirm the `slintx` crates.io package name is available and the publisher has the correct owner
  permissions.
- Review `cargo package --list` and run `cargo publish --dry-run` from a clean checkout.
- Publish only from a reviewed, tagged commit after CI passes; never place a crates.io token in
  the repository.

## Current limitation

The local checkout has no Git remote and `Guhuza/slintx` was not found on GitHub during the
repository audit. Therefore no repository URL, GitHub settings, branch rules, or status-check
configuration can be verified from this checkout. The issue-template config intentionally avoids
a repository-specific security-advisory link until the canonical repository is confirmed.
