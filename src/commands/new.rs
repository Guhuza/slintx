use anyhow::Result;

use crate::cli::NewArgs;
use crate::template;

pub fn run(args: NewArgs) -> Result<()> {
    template::create(args)?;
    Ok(())
}
