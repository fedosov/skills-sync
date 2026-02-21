use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

pub struct SyncWatchStream {
    watcher: RecommendedWatcher,
    rx: Receiver<notify::Result<notify::Event>>,
}

impl SyncWatchStream {
    pub fn new(paths: &[std::path::PathBuf]) -> notify::Result<Self> {
        let (tx, rx) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |result| {
            let _ = tx.send(result);
        })?;
        let mut watched = HashSet::new();

        for path in paths {
            if let Some((watch_path, mode)) = resolve_watch_target(path) {
                let key = format!("{}::{mode:?}", watch_path.display());
                if watched.insert(key) {
                    watcher.watch(&watch_path, mode)?;
                }
            }
        }

        Ok(Self { watcher, rx })
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Option<notify::Result<notify::Event>> {
        self.rx.recv_timeout(timeout).ok()
    }

    pub fn _watcher_ref(&self) -> &RecommendedWatcher {
        &self.watcher
    }
}

fn resolve_watch_target(path: &Path) -> Option<(PathBuf, RecursiveMode)> {
    if path.exists() {
        let mode = if path.is_dir() {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };
        return Some((path.to_path_buf(), mode));
    }

    for ancestor in path.ancestors().skip(1) {
        if ancestor.exists() {
            let mode = RecursiveMode::NonRecursive;
            return Some((ancestor.to_path_buf(), mode));
        }
    }

    None
}

pub fn default_watch_paths(home: &Path) -> Vec<std::path::PathBuf> {
    vec![
        home.join(".claude").join("skills"),
        home.join(".agents").join("skills"),
        home.join(".codex").join("skills"),
        home.join(".agents").join("subagents"),
        home.join(".claude").join("agents"),
        home.join(".cursor").join("agents"),
        home.join(".config").join("ai-agents").join("config.toml"),
        home.join(".codex").join("config.toml"),
        home.join(".claude.json"),
        home.join(".claude").join("settings.local.json"),
    ]
}

#[cfg(test)]
mod tests {
    use super::{default_watch_paths, resolve_watch_target};
    use notify::RecursiveMode;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn default_watch_paths_include_new_mcp_related_files() {
        let home = PathBuf::from("/tmp/home");
        let paths = default_watch_paths(&home);

        assert!(paths.iter().any(|path| path == &home.join(".claude.json")));
        assert!(paths
            .iter()
            .any(|path| path == &home.join(".config").join("ai-agents").join("config.toml")));
    }

    #[test]
    fn resolve_watch_target_falls_back_to_existing_parent_non_recursive_for_missing_file() {
        let temp = TempDir::new().expect("tempdir");
        let home = temp.path().join("home");
        fs::create_dir_all(&home).expect("create home");
        let missing_file = home.join(".claude.json");

        let resolved = resolve_watch_target(&missing_file).expect("watch target");
        assert_eq!(resolved.0, home);
        assert_eq!(resolved.1, RecursiveMode::NonRecursive);
    }
}
