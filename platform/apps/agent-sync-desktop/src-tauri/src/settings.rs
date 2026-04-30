use crate::skills_runner::SkillsCliScope;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const SETTINGS_VERSION: u32 = 2;
const SETTINGS_FILE_NAME: &str = "settings.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DotagentsScope {
    Project,
    #[default]
    User,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveProjectContext {
    pub mode: DotagentsScope,
    pub project_root: Option<String>,
}

impl Default for ActiveProjectContext {
    fn default() -> Self {
        Self {
            mode: DotagentsScope::User,
            project_root: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsWorkspaceState {
    #[serde(default)]
    pub scope: SkillsCliScope,
    #[serde(default)]
    pub active_agents: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_override: Option<String>,
    /// `true` once the user has interacted with the workspace at least once,
    /// so `active_agents = []` is no longer treated as "needs detection".
    #[serde(default)]
    pub initialized: bool,
}

impl Default for SkillsWorkspaceState {
    fn default() -> Self {
        Self {
            scope: SkillsCliScope::Global,
            active_agents: Vec::new(),
            version_override: None,
            initialized: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedSettings {
    pub version: u32,
    #[serde(default)]
    pub active_project_context: ActiveProjectContext,
    #[serde(default)]
    pub skills_workspace_state: SkillsWorkspaceState,
}

impl Default for PersistedSettings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            active_project_context: ActiveProjectContext::default(),
            skills_workspace_state: SkillsWorkspaceState::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SettingsStore {
    settings_path: PathBuf,
}

impl SettingsStore {
    pub fn new(settings_dir: impl Into<PathBuf>) -> Self {
        Self {
            settings_path: settings_dir.into().join(SETTINGS_FILE_NAME),
        }
    }

    pub fn load(&self) -> PersistedSettings {
        let Ok(contents) = fs::read_to_string(&self.settings_path) else {
            return PersistedSettings::default();
        };
        serde_json::from_str::<PersistedSettings>(&contents)
            .map(normalize_settings)
            .unwrap_or_default()
    }

    pub fn save(&self, settings: &PersistedSettings) -> Result<(), String> {
        if let Some(parent) = self.settings_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create settings directory {}: {error}",
                    parent.display()
                )
            })?;
        }

        let serialized = serde_json::to_string_pretty(&normalize_settings(settings.clone()))
            .map_err(|error| format!("failed to serialize settings: {error}"))?;
        fs::write(&self.settings_path, format!("{serialized}\n")).map_err(|error| {
            format!(
                "failed to write settings file {}: {error}",
                self.settings_path.display()
            )
        })
    }
}

fn normalize_settings(mut settings: PersistedSettings) -> PersistedSettings {
    settings.version = SETTINGS_VERSION;
    settings
}

#[cfg(test)]
mod tests {
    use super::{
        ActiveProjectContext, DotagentsScope, PersistedSettings, SettingsStore,
        SkillsWorkspaceState,
    };
    use crate::skills_runner::SkillsCliScope;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn load_returns_defaults_when_file_missing() {
        let temp = tempdir().expect("tempdir");
        let store = SettingsStore::new(temp.path());

        assert_eq!(store.load(), PersistedSettings::default());
    }

    #[test]
    fn save_and_load_round_trip() {
        let temp = tempdir().expect("tempdir");
        let store = SettingsStore::new(temp.path());
        let settings = PersistedSettings {
            version: 99,
            active_project_context: ActiveProjectContext {
                mode: DotagentsScope::Project,
                project_root: Some(String::from("/tmp/project")),
            },
            skills_workspace_state: SkillsWorkspaceState {
                scope: SkillsCliScope::Project,
                active_agents: vec![String::from("Claude Code")],
                version_override: Some(String::from("0.4.0")),
                initialized: true,
            },
        };

        store.save(&settings).expect("save settings");
        let loaded = store.load();

        assert_eq!(loaded.version, 2);
        assert_eq!(
            loaded.active_project_context.project_root.as_deref(),
            Some("/tmp/project")
        );
        assert_eq!(loaded.skills_workspace_state.scope, SkillsCliScope::Project);
        assert_eq!(
            loaded.skills_workspace_state.active_agents,
            vec![String::from("Claude Code")]
        );
        assert_eq!(
            loaded.skills_workspace_state.version_override.as_deref(),
            Some("0.4.0")
        );
        assert!(loaded.skills_workspace_state.initialized);
    }

    #[test]
    fn loads_v1_settings_with_defaulted_skills_workspace_state() {
        let temp = tempdir().expect("tempdir");
        let store = SettingsStore::new(temp.path());

        let v1_json = r#"{
            "version": 1,
            "activeProjectContext": {
                "mode": "project",
                "projectRoot": "/tmp/legacy"
            }
        }"#;
        fs::write(temp.path().join("settings.json"), v1_json).expect("write v1");

        let loaded = store.load();
        assert_eq!(loaded.version, 2);
        assert_eq!(
            loaded.active_project_context.project_root.as_deref(),
            Some("/tmp/legacy")
        );
        assert_eq!(
            loaded.skills_workspace_state,
            SkillsWorkspaceState::default()
        );
    }
}
