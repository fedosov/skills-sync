use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct CodexSubagentConfigEntry {
    pub scope: String,
    pub workspace: Option<String>,
    pub subagent_key: String,
    pub description: String,
    pub prompt: String,
    pub model: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum CodexSubagentRegistryError {
    #[error("Invalid home directory for Codex config: {0}")]
    InvalidHomeDirectory(String),
    #[error("Failed to write Codex subagent registry: {0}")]
    WriteFailed(String),
}

pub struct CodexSubagentRegistryWriter {
    home_directory: PathBuf,
    begin_marker: &'static str,
    end_marker: &'static str,
}

impl CodexSubagentRegistryWriter {
    pub fn new(home_directory: PathBuf) -> Self {
        Self {
            home_directory,
            begin_marker: "# agent-sync:subagents:begin",
            end_marker: "# agent-sync:subagents:end",
        }
    }

    pub fn write_managed_registries(
        &self,
        entries: &[CodexSubagentConfigEntry],
    ) -> Result<(), CodexSubagentRegistryError> {
        let home = self.home_directory.to_string_lossy();
        if !home.starts_with('/') && !home.contains(":\\") {
            return Err(CodexSubagentRegistryError::InvalidHomeDirectory(
                home.to_string(),
            ));
        }

        let mut by_config: BTreeMap<PathBuf, Vec<CodexSubagentConfigEntry>> = BTreeMap::new();
        for entry in entries {
            let config_path = if entry.scope == "project" {
                let Some(workspace) = &entry.workspace else {
                    continue;
                };
                PathBuf::from(workspace).join(".codex").join("config.toml")
            } else {
                self.home_directory.join(".codex").join("config.toml")
            };
            by_config
                .entry(config_path)
                .or_default()
                .push(entry.clone());
        }
        for stale_config in self.find_managed_config_candidates() {
            by_config.entry(stale_config).or_default();
        }

        for (config_path, group) in by_config {
            let base_dir = config_path.parent().ok_or_else(|| {
                CodexSubagentRegistryError::WriteFailed("invalid config path".into())
            })?;
            std::fs::create_dir_all(base_dir)
                .map_err(|e| CodexSubagentRegistryError::WriteFailed(e.to_string()))?;
            let agents_dir = base_dir.join("agents");
            std::fs::create_dir_all(&agents_dir)
                .map_err(|e| CodexSubagentRegistryError::WriteFailed(e.to_string()))?;

            let mut role_lines = Vec::new();
            for item in group {
                let cfg_file = agents_dir.join(format!("{}.toml", item.subagent_key));
                let cfg_rel = format!("agents/{}.toml", item.subagent_key);
                self.write_subagent_config(&cfg_file, &item)?;
                role_lines.push(format!("[agents.{}]", item.subagent_key));
                role_lines.push(format!(
                    "description = \"{}\"",
                    toml_escape(&item.description)
                ));
                role_lines.push(format!("config_file = \"{}\"", toml_escape(&cfg_rel)));
                role_lines.push(String::new());
            }
            if role_lines.last().is_some_and(|line| line.is_empty()) {
                role_lines.pop();
            }

            let existing = std::fs::read_to_string(&config_path).unwrap_or_default();
            let updated = self.upsert_managed_block(&existing, &role_lines.join("\n"));
            std::fs::write(config_path, updated)
                .map_err(|e| CodexSubagentRegistryError::WriteFailed(e.to_string()))?;
        }

        Ok(())
    }

    fn find_managed_config_candidates(&self) -> Vec<PathBuf> {
        let mut configs = std::collections::BTreeSet::new();
        let global = self.home_directory.join(".codex").join("config.toml");
        if has_subagent_managed_block(&global, self.begin_marker) {
            configs.insert(global);
        }

        self.collect_workspace_managed_configs(&self.home_directory.join("Dev"), 2, &mut configs);
        self.collect_workspace_managed_configs(
            &self.home_directory.join(".codex").join("worktrees"),
            3,
            &mut configs,
        );
        configs.into_iter().collect()
    }

    fn collect_workspace_managed_configs(
        &self,
        root: &Path,
        max_levels: usize,
        out: &mut std::collections::BTreeSet<PathBuf>,
    ) {
        if max_levels == 0 || !root.exists() {
            return;
        }

        let mut current_level = vec![root.to_path_buf()];
        for level in 0..max_levels {
            let mut next_level = Vec::new();
            for dir in current_level {
                let Ok(entries) = std::fs::read_dir(&dir) else {
                    continue;
                };
                for entry in entries.filter_map(Result::ok) {
                    let Ok(file_type) = entry.file_type() else {
                        continue;
                    };
                    if !file_type.is_dir() {
                        continue;
                    }
                    let child = entry.path();
                    let config_path = child.join(".codex").join("config.toml");
                    if has_subagent_managed_block(&config_path, self.begin_marker) {
                        out.insert(config_path);
                        continue;
                    }
                    if level + 1 < max_levels {
                        next_level.push(child);
                    }
                }
            }
            if next_level.is_empty() {
                break;
            }
            current_level = next_level;
        }
    }

    fn write_subagent_config(
        &self,
        path: &Path,
        entry: &CodexSubagentConfigEntry,
    ) -> Result<(), CodexSubagentRegistryError> {
        let mut lines = Vec::new();
        lines.push(format!(
            "developer_instructions = \"\"\"{}\"\"\"",
            escape_multiline_toml(&entry.prompt)
        ));
        if let Some(model) = &entry.model {
            lines.push(format!("model = \"{}\"", toml_escape(model)));
        }
        lines.push(String::new());
        std::fs::write(path, lines.join("\n"))
            .map_err(|e| CodexSubagentRegistryError::WriteFailed(e.to_string()))
    }

    fn upsert_managed_block(&self, current: &str, body: &str) -> String {
        let mut block = vec![self.begin_marker.to_string()];
        if body.trim().is_empty() {
            block.push("# No managed subagent entries".to_string());
        } else {
            block.push(body.to_string());
        }
        block.push(self.end_marker.to_string());
        let block = block.join("\n");
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
}

fn toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn escape_multiline_toml(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace("\"\"\"", "\\\"\\\"\\\"")
}

fn has_subagent_managed_block(path: &Path, begin_marker: &str) -> bool {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return false;
    };
    raw.contains(begin_marker)
}

#[cfg(test)]
mod tests {
    use super::{CodexSubagentConfigEntry, CodexSubagentRegistryWriter};

    #[test]
    fn write_managed_registries_ignores_deep_nested_codex_config() {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("home");
        std::fs::create_dir_all(&home).expect("home");

        let deep_cfg = home
            .join("Dev")
            .join("repo-a")
            .join("src")
            .join("nested")
            .join(".codex")
            .join("config.toml");
        std::fs::create_dir_all(deep_cfg.parent().expect("parent dir for deep codex config"))
            .expect("create deep config dir");
        let original = "\
custom = true

# agent-sync:subagents:begin
[agents.legacy]
description = \"Legacy\"
config_file = \"agents/legacy.toml\"
# agent-sync:subagents:end
";
        std::fs::write(&deep_cfg, original).expect("write deep config");

        let writer = CodexSubagentRegistryWriter::new(home);
        writer
            .write_managed_registries(&[])
            .expect("write registries");

        let current = std::fs::read_to_string(&deep_cfg).expect("read deep config");
        assert_eq!(current, original);
    }

    #[test]
    fn write_managed_registries_updates_workspace_level_codex_config() {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("home");
        std::fs::create_dir_all(&home).expect("home");

        let workspace_cfg = home
            .join("Dev")
            .join("owner")
            .join("repo-a")
            .join(".codex")
            .join("config.toml");
        std::fs::create_dir_all(
            workspace_cfg
                .parent()
                .expect("parent dir for workspace codex config"),
        )
        .expect("create workspace config dir");
        std::fs::write(
            &workspace_cfg,
            "\
# agent-sync:subagents:begin
[agents.legacy]
description = \"Legacy\"
config_file = \"agents/legacy.toml\"
# agent-sync:subagents:end
",
        )
        .expect("write workspace config");

        let writer = CodexSubagentRegistryWriter::new(home);
        writer
            .write_managed_registries(&[])
            .expect("write registries");

        let current = std::fs::read_to_string(&workspace_cfg).expect("read workspace config");
        assert!(current.contains("# agent-sync:subagents:begin"));
        assert!(current.contains("# No managed subagent entries"));
        assert!(current.contains("# agent-sync:subagents:end"));
    }

    #[test]
    fn write_managed_registries_escapes_backslashes_in_prompt() {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("home");
        std::fs::create_dir_all(&home).expect("home");

        let writer = CodexSubagentRegistryWriter::new(home.clone());
        writer
            .write_managed_registries(&[CodexSubagentConfigEntry {
                scope: String::from("global"),
                workspace: None,
                subagent_key: String::from("reviewer"),
                description: String::from("Review code"),
                prompt: String::from("Use C:\\temp\\repo and regex \\d+\\.md"),
                model: None,
            }])
            .expect("write registries");

        let subagent_cfg = home.join(".codex").join("agents").join("reviewer.toml");
        let raw = std::fs::read_to_string(subagent_cfg).expect("read reviewer config");
        assert!(raw.contains("C:\\\\temp\\\\repo"));
        assert!(raw.contains("\\\\d+\\\\.md"));
    }
}
