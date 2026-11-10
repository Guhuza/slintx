use anyhow::Result;

use crate::cargo;
use crate::cli::{BuildArgs, CheckArgs};

pub fn build(args: BuildArgs) -> Result<()> {
    cargo::invoke_build(&args.cargo, args.release)
}

pub fn check(args: CheckArgs) -> Result<()> {
    cargo::invoke_check(&args.cargo)
}
