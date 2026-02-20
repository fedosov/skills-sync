use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
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

        for path in paths {
            if path.exists() {
                watcher.watch(path, RecursiveMode::Recursive)?;
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

pub fn default_watch_roots(home: &Path) -> Vec<std::path::PathBuf> {
    vec![
        home.join(".claude").join("skills"),
        home.join(".agents").join("skills"),
        home.join(".codex").join("skills"),
    ]
}
