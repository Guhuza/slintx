use std::ffi::OsString;
use std::io::ErrorKind;
use std::path::Path;
use std::process::{Command, ExitStatus};

use anyhow::{Context, Result};
use command_group::{CommandGroup, GroupChild};

pub struct ManagedProcess {
    child: GroupChild,
    stopped: bool,
}

pub fn spawn_application(
    executable: &Path,
    args: &[OsString],
    current_dir: &Path,
) -> Result<ManagedProcess> {
    let mut command = Command::new(executable);
    command.args(args).current_dir(current_dir);
    let child = command
        .group_spawn()
        .with_context(|| format!("failed to start application {}", executable.display()))?;
    Ok(ManagedProcess {
        child,
        stopped: false,
    })
}

pub fn try_wait(process: &mut ManagedProcess) -> Result<Option<ExitStatus>> {
    process
        .child
        .try_wait()
        .context("failed to inspect application process")
}

pub fn stop(process: &mut ManagedProcess) -> Result<()> {
    if process.stopped {
        return Ok(());
    }
    match process.child.kill() {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::InvalidInput => {}
        Err(error) => return Err(error).context("failed to stop application process group"),
    }
    process
        .child
        .wait()
        .context("failed to wait for application process group to stop")?;
    process.stopped = true;
    Ok(())
}

impl Drop for ManagedProcess {
    fn drop(&mut self) {
        if !self.stopped
            && let Err(error) = stop(self)
        {
            eprintln!("failed to clean up application process group: {error:#}");
        }
    }
}
