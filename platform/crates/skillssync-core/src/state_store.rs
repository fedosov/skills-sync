use crate::error::SyncEngineError;
use crate::models::{SkillRecord, SyncState};
use crate::paths::SyncPaths;

#[derive(Debug, Clone)]
pub struct SyncStateStore {
    paths: SyncPaths,
}

impl Default for SyncStateStore {
    fn default() -> Self {
        Self {
            paths: SyncPaths::detect(),
        }
    }
}

impl SyncStateStore {
    pub fn new(paths: SyncPaths) -> Self {
        Self { paths }
    }

    pub fn load_state(&self) -> SyncState {
        let Ok(data) = std::fs::read(&self.paths.state_path) else {
            return SyncState::empty();
        };

        serde_json::from_slice(&data).unwrap_or_else(|_| SyncState::empty())
    }

    pub fn save_state(&self, state: &SyncState) -> Result<(), SyncEngineError> {
        self.paths
            .ensure_runtime_dir()
            .map_err(|e| SyncEngineError::io(&self.paths.runtime_directory, e))?;
        let mut payload = serde_json::to_vec_pretty(state)?;
        payload.push(b'\n');
        std::fs::write(&self.paths.state_path, payload)
            .map_err(|e| SyncEngineError::io(&self.paths.state_path, e))
    }

    pub fn top_skills(&self, state: &SyncState) -> Vec<SkillRecord> {
        let mut preferred: Vec<SkillRecord> = Vec::new();
        for id in &state.top_skills {
            if let Some(skill) = state.skills.iter().find(|skill| skill.id == *id) {
                preferred.push(skill.clone());
            }
            if preferred.len() >= 6 {
                return preferred;
            }
        }

        let mut fallback: Vec<SkillRecord> = state
            .skills
            .iter()
            .filter(|skill| !preferred.iter().any(|item| item.id == skill.id))
            .cloned()
            .collect();

        fallback.sort_by(|lhs, rhs| {
            if lhs.scope != rhs.scope {
                return lhs.scope.cmp(&rhs.scope);
            }
            lhs.name
                .to_ascii_lowercase()
                .cmp(&rhs.name.to_ascii_lowercase())
        });

        preferred.extend(fallback);
        preferred.truncate(6);
        preferred
    }

    pub fn paths(&self) -> &SyncPaths {
        &self.paths
    }
}
