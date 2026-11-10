# {{project_name}}

A desktop application built with Rust and Slint 1.18.1.

```text
cargo check
cargo run
```

The `.slint` UI is compiled by `slint-build` from `build.rs` and included by
`slint::include_modules!()` in the Rust application.
