# {{project_name}}

A Rust workspace with a Slint desktop application and a separate application-core library.

```text
cargo check --workspace
cargo run --package {{package_name}}-desktop
```

`apps/desktop/build.rs` compiles the Slint UI; `crates/app-core` contains UI-independent Rust
logic.
