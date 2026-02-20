use crate::models::{SkillLifecycleStatus, SkillRecord};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
struct RegistryEntry {
    path: String,
    enabled: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum CodexRegistryError {
    #[error("Invalid home directory for Codex config: {0}")]
    InvalidHomeDirectory(String),
    #[error("Failed to write Codex registry: {0}")]
    WriteFailed(String),
}

pub struct CodexSkillsRegistryWriter {
    home_directory: std::path::PathBuf,
    begin_marker: &'static str,
    end_marker: &'static str,
}

impl CodexSkillsRegistryWriter {
    pub fn new(home_directory: std::path::PathBuf) -> Self {
        Self {
            home_directory,
            begin_marker: "# skills-sync:begin",
            end_marker: "# skills-sync:end",
        }
    }

    pub fn write_managed_registry(&self, skills: &[SkillRecord]) -> Result<(), CodexRegistryError> {
        let home = self.home_directory.to_string_lossy();
        if !home.starts_with('/') && !home.contains(":\\") {
            return Err(CodexRegistryError::InvalidHomeDirectory(home.to_string()));
        }

        let entries = self.build_entries(skills);
        let config_path = self.home_directory.join(".codex").join("config.toml");
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CodexRegistryError::WriteFailed(e.to_string()))?;
        }
        let existing = std::fs::read_to_string(&config_path).unwrap_or_default();
        let updated = self.upsert_managed_block(&existing, &entries);
        std::fs::write(&config_path, updated)
            .map_err(|e| CodexRegistryError::WriteFailed(e.to_string()))
    }

    fn build_entries(&self, skills: &[SkillRecord]) -> Vec<RegistryEntry> {
        let mut unique = std::collections::HashSet::new();
        let mut ordered = Vec::new();

        for skill in skills {
            if skill.status != SkillLifecycleStatus::Active {
                continue;
            }

            if let Some(target) = preferred_agents_target(skill) {
                let standardized = Path::new(&target)
                    .components()
                    .as_path()
                    .to_string_lossy()
                    .to_string();
                if unique.insert(standardized.clone()) {
                    ordered.push(RegistryEntry {
                        path: standardized,
                        enabled: true,
                    });
                }
                continue;
            }

            if unique.insert(skill.canonical_source_path.clone()) {
                ordered.push(RegistryEntry {
                    path: skill.canonical_source_path.clone(),
                    enabled: true,
                });
            }
        }

        ordered.sort_by(|lhs, rhs| lhs.path.cmp(&rhs.path));
        ordered
    }

    fn upsert_managed_block(&self, current: &str, entries: &[RegistryEntry]) -> String {
        let block = self.managed_block(entries);
        if current.trim().is_empty() {
            return format!("{block}\n");
        }

        let normalized = current.replace("\r\n", "\n");
        if let Some(begin_index) = normalized.find(self.begin_marker) {
            if let Some(end_index) = normalized[begin_index..].find(self.end_marker) {
                let end_absolute = begin_index + end_index + self.end_marker.len();
                let prefix = normalized[..begin_index].trim_matches('\n');
                let suffix = normalized[end_absolute..].trim_matches('\n');

                return match (prefix.is_empty(), suffix.is_empty()) {
                    (true, true) => format!("{block}\n"),
                    (true, false) => format!("{block}\n\n{suffix}\n"),
                    (false, true) => format!("{prefix}\n\n{block}\n"),
                    (false, false) => format!("{prefix}\n\n{block}\n\n{suffix}\n"),
                };
            }
        }

        let trimmed = normalized.trim_matches('\n');
        format!("{trimmed}\n\n{block}\n")
    }

    fn managed_block(&self, entries: &[RegistryEntry]) -> String {
        let mut lines = vec![self.begin_marker.to_string()];
        if entries.is_empty() {
            lines.push("# No managed skill entries".to_string());
        } else {
            for entry in entries {
                lines.push("[[skills.config]]".to_string());
                lines.push(format!("path = \"{}\"", toml_escape(&entry.path)));
                lines.push(format!(
                    "enabled = {}",
                    if entry.enabled { "true" } else { "false" }
                ));
                lines.push(String::new());
            }
            if lines.last().is_some_and(|line| line.is_empty()) {
                lines.pop();
            }
        }
        lines.push(self.end_marker.to_string());
        lines.join("\n")
    }
}

fn preferred_agents_target(skill: &SkillRecord) -> Option<String> {
    let needle = format!("/.agents/skills/{}", skill.skill_key);
    skill
        .target_paths
        .iter()
        .find(|path| path.ends_with(&needle))
        .cloned()
}

fn toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
