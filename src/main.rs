mod cargo;
mod cli;
mod commands;
mod config;
mod process;
mod template;
mod watch;

use clap::Parser;
use cli::Cli;
use tracing_subscriber::EnvFilter;

fn main() {
    let filter = match EnvFilter::try_from_default_env() {
        Ok(filter) => filter,
        Err(error) if std::env::var_os("RUST_LOG").is_some() => {
            eprintln!("warning: invalid RUST_LOG filter ({error}); using `info`");
            EnvFilter::new("info")
        }
        Err(_) => EnvFilter::new("info"),
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    if let Err(error) = commands::run(Cli::parse()) {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}
