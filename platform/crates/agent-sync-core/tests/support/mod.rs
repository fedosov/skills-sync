use agent_sync_core::{
    McpServerRecord, SkillLifecycleStatus, SkillLocator, SkillRecord, SubagentRecord, SyncEngine,
    SyncEngineEnvironment, SyncPaths, SyncPreferencesStore, SyncState, SyncStateStore,
};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use tempfile::TempDir;

pub struct EngineHarness {
    temp: TempDir,
    engine: SyncEngine,
}

impl EngineHarness {
    pub fn new() -> Self {
        let temp = TempDir::new().expect("tempdir");
        let home = temp.path().join("home");
        let runtime = temp.path().join("runtime");
        let app_runtime = temp.path().join("app-runtime");
        fs::create_dir_all(&home).expect("home");
        fs::create_dir_all(&runtime).expect("runtime");
        fs::create_dir_all(&app_runtime).expect("app runtime");

        let env = SyncEngineEnvironment {
            home_directory: home.clone(),
            dev_root: home.join("Dev"),
            worktrees_root: home.join(".codex").join("worktrees"),
            runtime_directory: runtime,
        };

        let paths = SyncPaths::from_runtime(app_runtime);
        let store = SyncStateStore::new(paths.clone());
        let prefs = SyncPreferencesStore::new(paths);

        Self {
            temp,
            engine: SyncEngine::new(env, store, prefs),
        }
    }

    pub fn engine(&self) -> &SyncEngine {
        &self.engine
    }

    pub fn temp_dir(&self) -> &TempDir {
        &self.temp
    }

    pub fn app_settings_path(&self) -> PathBuf {
        self.temp
            .path()
            .join("app-runtime")
            .join("app-settings.json")
    }
}

pub fn write_skill(root: &Path, key: &str, body: &str) {
    let skill_path = root.join(key).join("SKILL.md");
    fs::create_dir_all(skill_path.parent().expect("parent")).expect("create parent");
    fs::write(skill_path, body).expect("write skill");
}

pub fn write_subagent(root: &Path, key: &str, body: &str) {
    let subagent_path = root.join(format!("{key}.md"));
    fs::create_dir_all(subagent_path.parent().expect("parent")).expect("create parent");
    fs::write(subagent_path, body).expect("write subagent");
}

pub fn write_text(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, body).expect("write file");
}

pub fn count_occurrences(body: &str, needle: &str) -> usize {
    body.match_indices(needle).count()
}

pub fn dotagents_env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub struct EnvVarRestore {
    key: &'static str,
    previous: Option<OsString>,
}

impl Drop for EnvVarRestore {
    fn drop(&mut self) {
        if let Some(value) = self.previous.take() {
            std::env::set_var(self.key, value);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

pub fn set_env_var_with_restore(key: &'static str, value: &Path) -> EnvVarRestore {
    let previous = std::env::var_os(key);
    std::env::set_var(key, value);
    EnvVarRestore { key, previous }
}

pub fn set_env_value_with_restore(key: &'static str, value: &str) -> EnvVarRestore {
    let previous = std::env::var_os(key);
    std::env::set_var(key, value);
    EnvVarRestore { key, previous }
}

pub fn unset_env_var_with_restore(key: &'static str) -> EnvVarRestore {
    let previous = std::env::var_os(key);
    std::env::remove_var(key);
    EnvVarRestore { key, previous }
}

pub fn find_skill(
    engine: &SyncEngine,
    skill_key: &str,
    status: Option<SkillLifecycleStatus>,
) -> SkillRecord {
    engine
        .find_skill(&SkillLocator {
            skill_key: String::from(skill_key),
            status,
        })
        .expect("skill exists")
}

pub fn find_mcp<'a>(
    state: &'a SyncState,
    server_key: &str,
    scope: &str,
    workspace: Option<&str>,
) -> &'a McpServerRecord {
    state
        .mcp_servers
        .iter()
        .find(|item| {
            item.server_key == server_key
                && item.scope == scope
                && item.workspace.as_deref() == workspace
        })
        .expect("mcp record exists")
}

pub fn find_subagent<'a>(
    state: &'a SyncState,
    subagent_key: &str,
    status: Option<SkillLifecycleStatus>,
) -> &'a SubagentRecord {
    state
        .subagents
        .iter()
        .find(|item| {
            item.subagent_key == subagent_key
                && status.map(|value| item.status == value).unwrap_or(true)
        })
        .expect("subagent exists")
}
