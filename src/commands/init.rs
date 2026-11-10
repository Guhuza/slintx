use anyhow::Result;

use crate::cli::InitArgs;
use crate::template;

pub fn run(args: InitArgs) -> Result<()> {
    template::init(args)
}
