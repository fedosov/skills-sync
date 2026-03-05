use crate::managed_block::{strip_managed_blocks, upsert_managed_block};
use crate::models::{SkillLifecycleStatus, SkillRecord};
use std::path::Path;

const AGENT_SYNC_BEGIN: &str = "# agent-sync:begin";
const AGENT_SYNC_END: &str = "# agent-sync:end";
const SKILLS_SYNC_BEGIN: &str = "# skills-sync:begin";
const SKILLS_SYNC_END: &str = "# skills-sync:end";
const SKILLS_MANAGED_MARKER_PAIRS: [(&str, &str); 2] = [
    (AGENT_SYNC_BEGIN, AGENT_SYNC_END),
    (SKILLS_SYNC_BEGIN, SKILLS_SYNC_END),
];

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
            begin_marker: AGENT_SYNC_BEGIN,
            end_marker: AGENT_SYNC_END,
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
        let unmanaged = strip_managed_blocks(&existing, &SKILLS_MANAGED_MARKER_PAIRS);
        let unmanaged = clean_orphaned_end_markers(&unmanaged, AGENT_SYNC_END);
        let unmanaged = clean_orphaned_end_markers(&unmanaged, SKILLS_SYNC_END);
        let updated = self.upsert_managed_registry(&unmanaged, &entries);
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

    fn upsert_managed_registry(&self, current: &str, entries: &[RegistryEntry]) -> String {
        let body = self.managed_block_body(entries);
        upsert_managed_block(current, self.begin_marker, self.end_marker, &body)
    }

    fn managed_block_body(&self, entries: &[RegistryEntry]) -> String {
        if entries.is_empty() {
            return "# No managed skill entries".to_string();
        }
        let config_array: Vec<toml::Value> = entries
            .iter()
            .map(|entry| {
                let mut item = toml::Table::new();
                item.insert("enabled".into(), toml::Value::Boolean(entry.enabled));
                item.insert("path".into(), toml::Value::String(entry.path.clone()));
                toml::Value::Table(item)
            })
            .collect();
        let mut skills = toml::Table::new();
        skills.insert("config".into(), toml::Value::Array(config_array));
        let mut root = toml::Table::new();
        root.insert("skills".into(), toml::Value::Table(skills));
        toml::to_string(&root)
            .expect("BUG: invalid TOML table")
            .trim_end()
            .to_string()
    }
}

/// Removes orphaned end markers that have no matching begin marker.
fn clean_orphaned_end_markers(text: &str, end_marker: &str) -> String {
    // Derive the begin marker from the end marker (e.g., "# agent-sync:end" → "# agent-sync:begin")
    let begin_marker = end_marker.replace(":end", ":begin");
    let mut result = Vec::new();
    let mut begin_count: usize = 0;
    let mut end_count: usize = 0;

    // First pass: count begin/end markers
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == begin_marker {
            begin_count += 1;
        } else if trimmed == end_marker {
            end_count += 1;
        }
    }

    // If there are more end markers than begin markers, there are orphans
    if end_count <= begin_count {
        return text.to_string();
    }

    // Second pass: skip orphaned end markers (those without a preceding begin)
    let mut pending_begins: usize = 0;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == begin_marker {
            pending_begins += 1;
            result.push(line);
        } else if trimmed == end_marker {
            if pending_begins > 0 {
                pending_begins -= 1;
                result.push(line);
            }
            // else: orphaned end marker — skip it
        } else {
            result.push(line);
        }
    }

    let mut out = result.join("\n");
    if text.ends_with('\n') && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn preferred_agents_target(skill: &SkillRecord) -> Option<String> {
    let needle = format!("/.agents/skills/{}", skill.skill_key);
    skill
        .target_paths
        .iter()
        .find(|path| path.ends_with(&needle))
        .cloned()
}
