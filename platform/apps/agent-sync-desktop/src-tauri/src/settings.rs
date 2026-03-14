use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const SETTINGS_VERSION: u32 = 1;
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
pub struct PersistedSettings {
    pub version: u32,
    pub active_project_context: ActiveProjectContext,
}

impl Default for PersistedSettings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            active_project_context: ActiveProjectContext::default(),
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
    use super::{ActiveProjectContext, DotagentsScope, PersistedSettings, SettingsStore};
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
        };

        store.save(&settings).expect("save settings");
        let loaded = store.load();

        assert_eq!(loaded.version, 1);
        assert_eq!(
            loaded.active_project_context.project_root.as_deref(),
            Some("/tmp/project")
        );
    }
}
