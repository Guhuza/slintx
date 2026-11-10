use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use tracing::{info, warn};

use crate::cargo;
use crate::cli::DevArgs;
use crate::config;
use crate::process;
use crate::watch::ProjectWatcher;

const DEBOUNCE: Duration = Duration::from_millis(250);

pub fn run(args: DevArgs) -> Result<()> {
    let manifest = config::resolve_manifest(args.cargo.manifest_path.as_deref())?;
    let root = config::project_root(&manifest)?.to_path_buf();
    let project_config = config::load(&root)?;
    let default_package = project_config
        .as_ref()
        .and_then(|config| config.default_package.as_deref());
    let watcher = ProjectWatcher::new(&root)?;
    let stopping = Arc::new(AtomicBool::new(false));
    let stop_handler = Arc::clone(&stopping);
    ctrlc::set_handler(move || stop_handler.store(true, Ordering::SeqCst))
        .context("failed to install Ctrl+C handler")?;

    info!("building project at {}", root.display());
    let mut application = None;
    while !stopping.load(Ordering::SeqCst) {
        match cargo::build_executable(&args.cargo, args.release, default_package, &stopping)? {
            Some(artifact) => {
                if let Some(mut previous) = application.take() {
                    process::stop(&mut previous)?;
                }
                let child = process::spawn_application(
                    &artifact.executable,
                    &args.app_args,
                    &artifact.project_root,
                )?;
                info!("running {}", artifact.executable.display());
                application = Some(child);
            }
            None if !stopping.load(Ordering::SeqCst) => {
                eprintln!("build failed; no executable was started from this build");
            }
            None => {}
        }

        if stopping.load(Ordering::SeqCst) {
            break;
        }
        match wait_for_change(&watcher, &root, &stopping, &mut application)? {
            true => {
                info!("change detected; rebuilding");
            }
            false => break,
        }
    }

    if let Some(mut child) = application {
        process::stop(&mut child)?;
    }
    if stopping.load(Ordering::SeqCst) {
        info!("stopped");
    }
    Ok(())
}

fn wait_for_change(
    watcher: &ProjectWatcher,
    root: &std::path::Path,
    stopping: &AtomicBool,
    application: &mut Option<process::ManagedProcess>,
) -> Result<bool> {
    let mut last_change = None;
    while !stopping.load(Ordering::SeqCst) {
        if let Some(child) = application.as_mut()
            && let Some(status) = process::try_wait(child)?
        {
            info!("application exited with {status}");
            *application = None;
        }

        let timeout = last_change
            .map(|last: Instant| DEBOUNCE.saturating_sub(last.elapsed()))
            .unwrap_or(Duration::from_millis(100))
            .min(Duration::from_millis(100));
        match watcher.events().recv_timeout(timeout) {
            Ok(Ok(event)) => {
                let relevant = watcher.relevant_event(&event, root);
                tracing::debug!(paths = ?event.paths, relevant, "filesystem event");
                if relevant {
                    last_change = Some(Instant::now());
                }
            }
            Ok(Err(error)) => warn!("filesystem watcher error: {error}"),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                bail!("filesystem watcher stopped unexpectedly");
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if last_change.is_some_and(|last| last.elapsed() >= DEBOUNCE) {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}
