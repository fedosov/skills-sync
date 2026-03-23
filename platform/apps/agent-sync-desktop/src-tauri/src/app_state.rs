use crate::dotagents_runner::{
    DotagentsCommandRequest, DotagentsCommandResult, DotagentsExecutionContext,
    DotagentsMcpListItem, DotagentsRunner, DotagentsSkillListItem,
};
use crate::dotagents_runtime::DotagentsRuntimeStatus;
use crate::open_path::open_path;
use crate::settings::{ActiveProjectContext, DotagentsScope, PersistedSettings, SettingsStore};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppContext {
    pub active_project_context: ActiveProjectContext,
    pub user_home: String,
    pub user_agents_dir: String,
    pub user_agents_toml_path: String,
    pub user_initialized: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_agents_toml_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_initialized: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct AppState {
    settings_store: SettingsStore,
    settings: Arc<Mutex<PersistedSettings>>,
    runner: DotagentsRunner,
    home_dir: PathBuf,
    command_lock: Arc<Mutex<()>>,
}

impl AppState {
    pub fn new(home_dir: PathBuf, settings_dir: PathBuf, runner: DotagentsRunner) -> Self {
        let settings_store = SettingsStore::new(settings_dir);
        let settings = settings_store.load();
        Self {
            settings_store,
            settings: Arc::new(Mutex::new(settings)),
            runner,
            home_dir,
            command_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn get_runtime_status(&self) -> DotagentsRuntimeStatus {
        self.runner.runtime_status()
    }

    pub fn get_app_context(&self) -> Result<AppContext, String> {
        let settings = self.load_settings()?;
        Ok(self.build_app_context(&settings))
    }

    pub fn set_scope(&self, scope: DotagentsScope) -> Result<AppContext, String> {
        let mut settings = self.load_settings()?;
        settings.active_project_context.mode = scope;
        self.save_settings(&settings)?;
        Ok(self.build_app_context(&settings))
    }

    pub fn set_project_root(&self, project_root: Option<String>) -> Result<AppContext, String> {
        let mut settings = self.load_settings()?;
        settings.active_project_context.project_root = match project_root {
            Some(root) => Some(normalize_project_root(&root)?.display().to_string()),
            None => None,
        };
        settings.active_project_context.mode = DotagentsScope::Project;
        self.save_settings(&settings)?;
        Ok(self.build_app_context(&settings))
    }

    pub fn list_skills(&self) -> Result<Vec<DotagentsSkillListItem>, String> {
        let Some(context) = self.active_execution_context_for_read()? else {
            return Ok(Vec::new());
        };
        self.runner.list_skills(&context)
    }

    pub fn list_mcp_servers(&self) -> Result<Vec<DotagentsMcpListItem>, String> {
        let Some(context) = self.active_execution_context_for_read()? else {
            return Ok(Vec::new());
        };
        self.runner.list_mcp_servers(&context)
    }

    pub fn run_dotagents_command(
        &self,
        request: DotagentsCommandRequest,
    ) -> Result<DotagentsCommandResult, String> {
        let settings = self.load_settings()?;
        let fallback_context = self.fallback_execution_context(&settings);
        let context = match self.active_execution_context_for_write(&settings) {
            Ok(context) => context,
            Err(error) => {
                return self
                    .runner
                    .preflight_failure_result(&request, &fallback_context, error)
            }
        };
        let _guard = self
            .command_lock
            .lock()
            .map_err(|e| format!("failed to lock dotagents command runner: {e}"))?;
        self.runner.run_command(&context, &request)
    }

    pub fn open_agents_toml(&self) -> Result<(), String> {
        let settings = self.load_settings()?;
        match settings.active_project_context.mode {
            DotagentsScope::User => open_path(&self.user_agents_toml_path()),
            DotagentsScope::Project => {
                let Some(root) = settings.active_project_context.project_root.as_deref() else {
                    return Err(String::from("select a project folder first"));
                };
                open_path(&PathBuf::from(root).join("agents.toml"))
            }
        }
    }

    pub fn open_agents_dir(&self) -> Result<(), String> {
        let settings = self.load_settings()?;
        match settings.active_project_context.mode {
            DotagentsScope::User => open_path(&self.user_agents_dir()),
            DotagentsScope::Project => {
                let Some(root) = settings.active_project_context.project_root.as_deref() else {
                    return Err(String::from("select a project folder first"));
                };
                let root_path = PathBuf::from(root);
                let agents_dir = root_path.join(".agents");
                if agents_dir.exists() {
                    open_path(&agents_dir)
                } else {
                    open_path(&root_path)
                }
            }
        }
    }

    pub fn open_user_home(&self) -> Result<(), String> {
        open_path(&self.home_dir)
    }

    #[cfg(test)]
    pub(crate) fn active_execution_context(
        &self,
    ) -> Result<Option<DotagentsExecutionContext>, String> {
        self.active_execution_context_for_read()
    }

    fn load_settings(&self) -> Result<PersistedSettings, String> {
        self.settings
            .lock()
            .map(|guard| guard.clone())
            .map_err(|e| format!("failed to read app settings: {e}"))
    }

    fn save_settings(&self, settings: &PersistedSettings) -> Result<(), String> {
        self.settings_store.save(settings)?;
        let mut guard = self
            .settings
            .lock()
            .map_err(|e| format!("failed to write app settings: {e}"))?;
        *guard = settings.clone();
        Ok(())
    }

    fn build_app_context(&self, settings: &PersistedSettings) -> AppContext {
        let project_root = settings
            .active_project_context
            .project_root
            .as_deref()
            .map(PathBuf::from);
        let project_agents_toml_path = project_root
            .as_ref()
            .map(|root| root.join("agents.toml").display().to_string());
        let project_initialized = project_root
            .as_ref()
            .map(|root| root.join("agents.toml").exists());

        AppContext {
            active_project_context: settings.active_project_context.clone(),
            user_home: self.home_dir.display().to_string(),
            user_agents_dir: self.user_agents_dir().display().to_string(),
            user_agents_toml_path: self.user_agents_toml_path().display().to_string(),
            user_initialized: self.user_agents_toml_path().exists(),
            project_agents_toml_path,
            project_initialized,
        }
    }

    fn active_execution_context_for_read(
        &self,
    ) -> Result<Option<DotagentsExecutionContext>, String> {
        let settings = self.load_settings()?;
        match settings.active_project_context.mode {
            DotagentsScope::User => {
                if !self.user_agents_toml_path().exists() {
                    return Ok(None);
                }
                Ok(Some(DotagentsExecutionContext {
                    scope: DotagentsScope::User,
                    cwd: self.home_dir.clone(),
                }))
            }
            DotagentsScope::Project => {
                let Some(root) = settings.active_project_context.project_root.as_deref() else {
                    return Ok(None);
                };
                let root_path = PathBuf::from(root);
                if !root_path.join("agents.toml").exists() {
                    return Ok(None);
                }
                Ok(Some(DotagentsExecutionContext {
                    scope: DotagentsScope::Project,
                    cwd: root_path,
                }))
            }
        }
    }

    fn active_execution_context_for_write(
        &self,
        settings: &PersistedSettings,
    ) -> Result<DotagentsExecutionContext, String> {
        match settings.active_project_context.mode {
            DotagentsScope::User => {
                if self.user_agents_toml_path().exists() {
                    return Ok(DotagentsExecutionContext {
                        scope: DotagentsScope::User,
                        cwd: self.home_dir.clone(),
                    });
                }
                Err(format!(
                    "No user agents.toml found at {}. Initialize dotagents manually first.",
                    self.user_agents_toml_path().display()
                ))
            }
            DotagentsScope::Project => {
                let Some(root) = settings.active_project_context.project_root.as_deref() else {
                    return Err(String::from("Select a project folder first."));
                };

                let context = DotagentsExecutionContext {
                    scope: DotagentsScope::Project,
                    cwd: PathBuf::from(root),
                };
                if context.cwd.join("agents.toml").exists() {
                    return Ok(context);
                }
                Err(String::from(
                    "The selected project folder does not contain agents.toml.",
                ))
            }
        }
    }

    fn fallback_execution_context(
        &self,
        settings: &PersistedSettings,
    ) -> DotagentsExecutionContext {
        match settings.active_project_context.mode {
            DotagentsScope::User => DotagentsExecutionContext {
                scope: DotagentsScope::User,
                cwd: self.home_dir.clone(),
            },
            DotagentsScope::Project => {
                if let Some(root) = settings.active_project_context.project_root.as_deref() {
                    return DotagentsExecutionContext {
                        scope: DotagentsScope::Project,
                        cwd: PathBuf::from(root),
                    };
                }
                DotagentsExecutionContext {
                    scope: DotagentsScope::Project,
                    cwd: self.home_dir.clone(),
                }
            }
        }
    }

    fn user_agents_dir(&self) -> PathBuf {
        self.home_dir.join(".agents")
    }

    fn user_agents_toml_path(&self) -> PathBuf {
        self.user_agents_dir().join("agents.toml")
    }
}

fn normalize_project_root(value: &str) -> Result<PathBuf, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(String::from("project root cannot be empty"));
    }

    let path = Path::new(trimmed);
    if !path.is_absolute() {
        return Err(String::from("project root must be an absolute path"));
    }
    if !path.is_dir() {
        return Err(format!(
            "project root is not a directory: {}",
            path.display()
        ));
    }

    std::fs::canonicalize(path)
        .map_err(|error| format!("failed to resolve {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::AppState;
    use crate::dotagents_runner::DotagentsRunner;
    use crate::dotagents_runtime::DotagentsRuntimeManager;
    use crate::settings::DotagentsScope;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn project_mode_uses_selected_project_root_as_cwd() {
        let temp = tempdir().expect("tempdir");
        let home_dir = temp.path().join("home");
        let settings_dir = temp.path().join("settings");
        let project_root = temp.path().join("project");
        fs::create_dir_all(&home_dir).expect("home");
        fs::create_dir_all(&settings_dir).expect("settings");
        fs::create_dir_all(&project_root).expect("project");
        fs::write(
            project_root.join("agents.toml"),
            "version = 1\nskills = []\n",
        )
        .expect("agents.toml");

        let state = AppState::new(
            home_dir.clone(),
            settings_dir,
            DotagentsRunner::new(home_dir, DotagentsRuntimeManager::new()),
        );
        state
            .set_project_root(Some(project_root.display().to_string()))
            .expect("set project root");
        state.set_scope(DotagentsScope::Project).expect("set scope");

        let context = state
            .active_execution_context()
            .expect("context")
            .expect("project context");

        assert_eq!(
            context.cwd,
            project_root.canonicalize().expect("canonical project")
        );
    }
}
