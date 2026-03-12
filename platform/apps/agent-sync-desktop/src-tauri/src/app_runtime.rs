use agent_sync_core::{watch::SyncWatchStream, SyncEngine};
use serde::Serialize;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const AUTO_WATCH_DEBOUNCE_MS: u64 = 800;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RuntimeControls {
    pub(crate) allow_filesystem_changes: bool,
    pub(crate) auto_watch_active: bool,
}

#[derive(Debug, Default)]
struct WatchRuntime {
    active: bool,
    stop_tx: Option<mpsc::Sender<()>>,
    handle: Option<JoinHandle<()>>,
}

#[derive(Debug, Clone)]
pub(crate) struct AppRuntime {
    watch: Arc<Mutex<WatchRuntime>>,
    sync_lock: Arc<Mutex<()>>,
}

impl AppRuntime {
    pub(crate) fn with_sync_lock<T, F>(&self, operation: F) -> Result<T, String>
    where
        F: FnOnce() -> Result<T, String>,
    {
        let _guard = self
            .sync_lock
            .lock()
            .map_err(|_| String::from("internal lock error"))?;
        operation()
    }

    pub(crate) fn runtime_controls(&self, engine: &SyncEngine) -> RuntimeControls {
        let auto_watch_active = self.watch.lock().map(|state| state.active).unwrap_or(false);
        RuntimeControls {
            allow_filesystem_changes: engine.allow_filesystem_changes(),
            auto_watch_active,
        }
    }

    pub(crate) fn enable_auto_watch_and_initial_sync(
        &self,
        engine: &SyncEngine,
    ) -> Result<(), String> {
        if !engine.allow_filesystem_changes() {
            return Ok(());
        }

        self.with_sync_lock(|| {
            engine
                .run_sync(agent_sync_core::SyncTrigger::Manual)
                .map(|_| ())
                .map_err(|e| e.to_string())
        })?;

        if !engine.allow_filesystem_changes() {
            return Ok(());
        }

        self.start_auto_watch(engine)?;

        if !engine.allow_filesystem_changes() {
            self.stop_auto_watch();
        }

        Ok(())
    }

    pub(crate) fn stop_auto_watch(&self) {
        let mut handle_to_join: Option<JoinHandle<()>> = None;
        if let Ok(mut state) = self.watch.lock() {
            if !state.active {
                return;
            }
            if let Some(stop_tx) = state.stop_tx.take() {
                let _ = stop_tx.send(());
            }
            handle_to_join = state.handle.take();
            state.active = false;
        }

        if let Some(handle) = handle_to_join {
            let _ = handle.join();
        }
    }

    pub(crate) fn set_allow_filesystem_changes_with<F>(
        &self,
        engine: &SyncEngine,
        allow: bool,
        enable_when_allowed: F,
    ) -> Result<RuntimeControls, String>
    where
        F: Fn(&AppRuntime, &SyncEngine) -> Result<(), String>,
    {
        engine
            .set_allow_filesystem_changes(allow)
            .map_err(|error| error.to_string())?;

        if allow {
            if let Err(error) = enable_when_allowed(self, engine) {
                self.stop_auto_watch();
                if let Err(rollback_error) = engine.set_allow_filesystem_changes(false) {
                    return Err(format!(
                        "{error}; failed to revert filesystem write mode: {rollback_error}"
                    ));
                }
                return Err(error);
            }
        } else {
            self.stop_auto_watch();
        }

        Ok(self.runtime_controls(engine))
    }

    pub(crate) fn set_allow_filesystem_changes(
        &self,
        engine: &SyncEngine,
        allow: bool,
    ) -> Result<RuntimeControls, String> {
        self.set_allow_filesystem_changes_with(
            engine,
            allow,
            AppRuntime::enable_auto_watch_and_initial_sync,
        )
    }

    fn start_auto_watch(&self, engine: &SyncEngine) -> Result<(), String> {
        if self.watch.lock().map(|state| state.active).unwrap_or(false) {
            return Ok(());
        }

        let watch_paths = engine.watch_paths();
        let stream = SyncWatchStream::new(&watch_paths)
            .map_err(|error| format!("failed to start filesystem watcher: {error}"))?;
        let sync_lock = Arc::clone(&self.sync_lock);
        let thread_engine = engine.clone();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();

        let handle = std::thread::spawn(move || {
            let mut pending_since: Option<Instant> = None;

            loop {
                if stop_rx.try_recv().is_ok() {
                    break;
                }

                match stream.recv_timeout(Duration::from_millis(250)) {
                    Some(Ok(_)) => {
                        pending_since = Some(Instant::now());
                    }
                    Some(Err(_)) => {}
                    None => {}
                }

                let should_sync = pending_since
                    .map(|started| {
                        started.elapsed() >= Duration::from_millis(AUTO_WATCH_DEBOUNCE_MS)
                    })
                    .unwrap_or(false);
                if !should_sync {
                    continue;
                }

                pending_since = None;
                if let Ok(_guard) = sync_lock.lock() {
                    let _ = thread_engine.run_sync(agent_sync_core::SyncTrigger::AutoFilesystem);
                }
            }
        });

        if let Ok(mut state) = self.watch.lock() {
            state.stop_tx = Some(stop_tx);
            state.handle = Some(handle);
            state.active = true;
            return Ok(());
        }

        Err(String::from("failed to update watcher runtime state"))
    }
}

impl Default for AppRuntime {
    fn default() -> Self {
        Self {
            watch: Arc::new(Mutex::new(WatchRuntime::default())),
            sync_lock: Arc::new(Mutex::new(())),
        }
    }
}
