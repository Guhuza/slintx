mod add;
mod build;
mod dev;
mod doctor;
mod init;
mod mobile;
mod new;
mod template;

use anyhow::Result;

use crate::cli::{Cli, Command};

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        None => {
            use clap::CommandFactory;
            let mut command = Cli::command();
            command.print_help()?;
            println!();
            Ok(())
        }
        Some(Command::New(args)) => new::run(args),
        Some(Command::Init(args)) => init::run(args),
        Some(Command::Add(args)) => add::run(args),
        Some(Command::Template(args)) => template::run(args),
        Some(Command::Dev(args)) => dev::run(args),
        Some(Command::Build(args)) => build::build(args),
        Some(Command::Check(args)) => build::check(args),
        Some(Command::Doctor(args)) => doctor::run(args),
        Some(Command::Android(args)) => mobile::android(args),
        Some(Command::Ios(args)) => mobile::ios(args),
    }
}
