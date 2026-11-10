use anyhow::Result;

use crate::cli::{TemplateArgs, TemplateCommand};
use crate::template;

pub fn run(args: TemplateArgs) -> Result<()> {
    match args.command {
        TemplateCommand::List => template::list(),
        TemplateCommand::Add(args) => template::add_external_template(args),
    }
}
