use std::fs;
use std::path::{Component, Path};
use std::sync::mpsc::{self, Receiver};

use anyhow::{Context, Result};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

pub struct ProjectWatcher {
    _watcher: RecommendedWatcher,
    events: Receiver<notify::Result<Event>>,
}

impl ProjectWatcher {
    pub fn new(root: &Path) -> Result<Self> {
        let (sender, events) = mpsc::channel();
        let mut watcher = RecommendedWatcher::new(
            move |event| {
                let _ = sender.send(event);
            },
            Config::default(),
        )
        .context("failed to create filesystem watcher")?;

        watcher
            .watch(root, RecursiveMode::NonRecursive)
            .with_context(|| format!("failed to watch project directory {}", root.display()))?;
        for directory in fs::read_dir(root)
            .with_context(|| format!("failed to read project directory {}", root.display()))?
        {
            let directory = directory?;
            if !directory.file_type()?.is_dir() || ignored_component(&directory.file_name()) {
                continue;
            }
            watcher
                .watch(&directory.path(), RecursiveMode::Recursive)
                .with_context(|| format!("failed to watch {}", directory.path().display()))?;
        }
        Ok(Self {
            _watcher: watcher,
            events,
        })
    }

    pub fn events(&self) -> &Receiver<notify::Result<Event>> {
        &self.events
    }

    pub fn relevant_event(&self, event: &Event, root: &Path) -> bool {
        event.paths.iter().any(|path| is_relevant_path(path, root))
    }
}

pub fn is_relevant_path(path: &Path, root: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    let components: Vec<Component<'_>> = relative.components().collect();
    if components.is_empty()
        || components
            .iter()
            .any(|component| ignored_component(component.as_os_str()))
    {
        return false;
    }
    if components.len() == 1 {
        let name = components[0].as_os_str();
        return name == "Cargo.toml"
            || name == "Cargo.lock"
            || name == "build.rs"
            || name == "slintx.toml"
            || Path::new(name)
                .extension()
                .is_some_and(|ext| ext == "slint");
    }
    let watched_tree = components.iter().any(|component| {
        let name = component.as_os_str();
        name == "src"
            || name == "ui"
            || name == "assets"
            || name == "apps"
            || name == "crates"
            || name == ".cargo"
    });
    watched_tree
        || Path::new(relative)
            .extension()
            .is_some_and(|extension| extension == "slint" || extension == "rs")
}

fn ignored_component(component: &std::ffi::OsStr) -> bool {
    component.eq_ignore_ascii_case("target")
        || component.eq_ignore_ascii_case(".git")
        || component.eq_ignore_ascii_case(".slintx")
        || component.eq_ignore_ascii_case("node_modules")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::is_relevant_path;

    #[test]
    fn watches_ui_rust_and_assets_but_not_build_outputs() {
        let root = Path::new("project");
        assert!(is_relevant_path(&root.join("ui").join("app.slint"), root));
        assert!(is_relevant_path(&root.join("src").join("main.rs"), root));
        assert!(is_relevant_path(
            &root.join("assets").join("icon.png"),
            root
        ));
        assert!(!is_relevant_path(
            &root.join("target").join("debug").join("app.exe"),
            root
        ));
        assert!(!is_relevant_path(&root.join(".git").join("index"), root));
    }
}
