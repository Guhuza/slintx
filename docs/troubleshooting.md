# Troubleshooting

## Cargo or Slint build errors

Run `slintx doctor` and inspect the original Cargo diagnostics. Confirm that both `slint` and
`slint-build` use the template's Slint 1.18.1 version. Slint UI syntax is compiled by the
official Slint build crate during Cargo's build-script phase.

On Linux, installing a desktop build may also require the system packages `libfontconfig1-dev`,
`libx11-dev`, `libxcb-shape0-dev`, `libxcb-xfixes0-dev`, `libxkbcommon-dev`,
`libxkbcommon-x11-dev`, and `libwayland-dev`.

## The app did not restart

SlintX starts a replacement only after Cargo succeeds and reports a current executable. A
failed build intentionally does not launch a possibly stale binary. Correct the compiler error
and save a watched input again, or rerun `slintx dev`.

## File changes are not detected

Run `slintx dev` from the project directory and check that the changed path is inside the
project's watched source/UI/assets trees. Filesystem notification APIs can behave differently
on network mounts, some WSL/VM shared folders, and container volumes. Use a local checkout or
run the CLI on the host that owns the files if events are unavailable.

## Process cleanup

Ctrl+C stops the managed application process group. If the OS denies process termination,
SlintX reports the cleanup error rather than claiming that the application exited.

## Android and iOS

If `slintx android` or `slintx ios` cannot find `x`, install xbuild with `cargo install xbuild`.
Run `slintx doctor --target <rust-target>` and follow the reported missing-tool suggestions.
Android builds require Rust targets plus Android SDK/NDK, CMake, JDK, and platform-tools. iOS
builds require macOS, Xcode, and the matching Apple Rust target. A successful Cargo cross-build
does not itself produce a signed or installable mobile package.
