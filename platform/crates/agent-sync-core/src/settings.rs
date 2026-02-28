use crate::error::SyncEngineError;
use crate::paths::SyncPaths;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncAppSettings {
    pub version: u32,
    #[serde(default, rename = "auto_migrate_to_canonical_source")]
    pub auto_migrate_to_canonical_source: bool,
    #[serde(default, rename = "allow_filesystem_changes")]
    pub allow_filesystem_changes: bool,
    #[serde(default, rename = "workspace_discovery_roots")]
    pub workspace_discovery_roots: Vec<String>,
    #[serde(default, rename = "window_state")]
    pub window_state: Option<AppWindowState>,
    #[serde(default, rename = "ui_state")]
    pub ui_state: Option<AppUiState>,
}

impl Default for SyncAppSettings {
    fn default() -> Self {
        Self {
            version: 2,
            auto_migrate_to_canonical_source: false,
            allow_filesystem_changes: false,
            workspace_discovery_roots: Vec::new(),
            window_state: None,
            ui_state: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppWindowState {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(rename = "is_maximized")]
    pub is_maximized: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppUiState {
    #[serde(rename = "sidebar_width")]
    pub sidebar_width: Option<f64>,
    #[serde(rename = "scope_filter")]
    pub scope_filter: String,
    #[serde(rename = "search_text")]
    pub search_text: String,
    #[serde(rename = "selected_skill_ids")]
    pub selected_skill_ids: Vec<String>,
    #[serde(default, rename = "starred_skill_ids")]
    pub starred_skill_ids: Vec<String>,
}

impl Default for AppUiState {
    fn default() -> Self {
        Self {
            sidebar_width: None,
            scope_filter: String::from("all"),
            search_text: String::new(),
            selected_skill_ids: Vec::new(),
            starred_skill_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SyncPreferencesStore {
    paths: SyncPaths,
}

impl Default for SyncPreferencesStore {
    fn default() -> Self {
        Self {
            paths: SyncPaths::detect(),
        }
    }
}

impl SyncPreferencesStore {
    pub fn new(paths: SyncPaths) -> Self {
        Self { paths }
    }

    pub fn load_settings(&self) -> SyncAppSettings {
        let Ok(data) = std::fs::read(&self.paths.app_settings_path) else {
            return SyncAppSettings::default();
        };

        serde_json::from_slice(&data).unwrap_or_else(|_| SyncAppSettings::default())
    }

    pub fn save_settings(&self, settings: &SyncAppSettings) -> Result<(), SyncEngineError> {
        self.paths
            .ensure_runtime_dir()
            .map_err(|e| SyncEngineError::io(&self.paths.runtime_directory, e))?;

        let normalized = SyncAppSettings {
            version: 2,
            auto_migrate_to_canonical_source: settings.auto_migrate_to_canonical_source,
            allow_filesystem_changes: settings.allow_filesystem_changes,
            workspace_discovery_roots: settings.workspace_discovery_roots.clone(),
            window_state: settings.window_state.clone(),
            ui_state: settings.ui_state.clone(),
        };

        let mut payload = serde_json::to_vec_pretty(&normalized)?;
        payload.push(b'\n');
        std::fs::write(&self.paths.app_settings_path, payload)
            .map_err(|e| SyncEngineError::io(&self.paths.app_settings_path, e))
    }

    pub fn paths(&self) -> &SyncPaths {
        &self.paths
    }
}

#[cfg(test)]
mod tests {
    use super::SyncAppSettings;

    #[test]
    fn app_ui_state_defaults_missing_starred_skill_ids() {
        let raw = r#"{
          "version": 2,
          "auto_migrate_to_canonical_source": false,
          "allow_filesystem_changes": false,
          "workspace_discovery_roots": [],
          "window_state": null,
          "ui_state": {
            "sidebar_width": null,
            "scope_filter": "all",
            "search_text": "",
            "selected_skill_ids": []
          }
        }"#;

        let parsed: SyncAppSettings = serde_json::from_str(raw).expect("parse settings");
        assert!(!parsed.allow_filesystem_changes);
        let starred = parsed.ui_state.expect("ui state").starred_skill_ids;
        assert!(starred.is_empty());
    }

    #[test]
    fn settings_default_allow_filesystem_changes_when_missing() {
        let raw = r#"{
          "version": 2,
          "auto_migrate_to_canonical_source": false,
          "workspace_discovery_roots": [],
          "window_state": null,
          "ui_state": null
        }"#;

        let parsed: SyncAppSettings = serde_json::from_str(raw).expect("parse settings");
        assert!(!parsed.allow_filesystem_changes);
    }
}
