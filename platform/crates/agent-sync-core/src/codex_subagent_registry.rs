use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

const AGENT_SYNC_SUBAGENTS_BEGIN: &str = "# agent-sync:subagents:begin";
const AGENT_SYNC_SUBAGENTS_END: &str = "# agent-sync:subagents:end";
const SKILLS_SYNC_SUBAGENTS_BEGIN: &str = "# skills-sync:subagents:begin";
const SKILLS_SYNC_SUBAGENTS_END: &str = "# skills-sync:subagents:end";
const SUBAGENT_MANAGED_MARKER_PAIRS: [(&str, &str); 2] = [
    (AGENT_SYNC_SUBAGENTS_BEGIN, AGENT_SYNC_SUBAGENTS_END),
    (SKILLS_SYNC_SUBAGENTS_BEGIN, SKILLS_SYNC_SUBAGENTS_END),
];
const SUBAGENT_MANAGED_BEGIN_MARKERS: [&str; 2] =
    [AGENT_SYNC_SUBAGENTS_BEGIN, SKILLS_SYNC_SUBAGENTS_BEGIN];

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
            begin_marker: AGENT_SYNC_SUBAGENTS_BEGIN,
            end_marker: AGENT_SYNC_SUBAGENTS_END,
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

            let mut deduped_group = Vec::new();
            let mut seen_keys = HashSet::new();
            for item in group {
                if seen_keys.insert(item.subagent_key.clone()) {
                    deduped_group.push(item);
                }
            }

            let existing = std::fs::read_to_string(&config_path).unwrap_or_default();
            let unmanaged = strip_managed_blocks(&existing, &SUBAGENT_MANAGED_MARKER_PAIRS);
            let unmanaged_agents = extract_unmanaged_agent_keys(&unmanaged);
            let mut agents_table = toml::Table::new();
            let mut skipped_keys: BTreeSet<String> = BTreeSet::new();
            for item in deduped_group {
                if unmanaged_agents.contains(&item.subagent_key) {
                    skipped_keys.insert(item.subagent_key);
                    continue;
                }

                let cfg_file = agents_dir.join(format!("{}.toml", item.subagent_key));
                let cfg_rel = format!("agents/{}.toml", item.subagent_key);
                self.write_subagent_config(&cfg_file, &item)?;
                let mut agent_entry = toml::Table::new();
                agent_entry.insert(
                    "description".into(),
                    toml::Value::String(item.description.clone()),
                );
                agent_entry.insert("config_file".into(), toml::Value::String(cfg_rel));
                agents_table.insert(item.subagent_key.clone(), toml::Value::Table(agent_entry));
            }

            let role_toml = if agents_table.is_empty() {
                String::new()
            } else {
                let mut root = toml::Table::new();
                root.insert("agents".into(), toml::Value::Table(agents_table));
                toml::to_string(&root)
                    .expect("BUG: invalid TOML table")
                    .trim_end()
                    .to_string()
            };

            let mut body_lines = Vec::new();
            for key in skipped_keys {
                body_lines.push(format!(
                    "# Skipped managed subagent '{}' because unmanaged agents.{} table already exists",
                    key, key
                ));
            }
            if !body_lines.is_empty() && !role_toml.is_empty() {
                body_lines.push(String::new());
            }
            if !role_toml.is_empty() {
                body_lines.push(role_toml);
            }

            let updated = self.upsert_managed_block(&unmanaged, &body_lines.join("\n"));
            toml::from_str::<toml::Table>(&updated).map_err(|error| {
                CodexSubagentRegistryError::WriteFailed(format!(
                    "generated invalid TOML for {}: {error}",
                    config_path.display()
                ))
            })?;
            std::fs::write(config_path, updated)
                .map_err(|e| CodexSubagentRegistryError::WriteFailed(e.to_string()))?;
        }

        Ok(())
    }

    fn find_managed_config_candidates(&self) -> Vec<PathBuf> {
        let mut configs = std::collections::BTreeSet::new();
        let global = self.home_directory.join(".codex").join("config.toml");
        if has_subagent_managed_block(&global, &SUBAGENT_MANAGED_BEGIN_MARKERS) {
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
                    if has_subagent_managed_block(&config_path, &SUBAGENT_MANAGED_BEGIN_MARKERS) {
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
        let mut table = toml::Table::new();
        table.insert(
            "developer_instructions".into(),
            toml::Value::String(entry.prompt.clone()),
        );
        if let Some(model) = &entry.model {
            table.insert("model".into(), toml::Value::String(model.clone()));
        }
        let content = toml::to_string(&table).expect("BUG: invalid TOML table");
        std::fs::write(path, content)
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

fn has_subagent_managed_block(path: &Path, begin_markers: &[&str]) -> bool {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return false;
    };
    begin_markers.iter().any(|begin| raw.contains(begin))
}

fn extract_unmanaged_agent_keys(raw: &str) -> BTreeSet<String> {
    if let Ok(parsed) = toml::from_str::<toml::Value>(raw) {
        if let Some(root) = parsed.as_table() {
            if let Some(agents) = root.get("agents").and_then(toml::Value::as_table) {
                let mut keys = BTreeSet::new();
                for key in agents.keys() {
                    let trimmed = key.trim();
                    if !trimmed.is_empty() {
                        keys.insert(trimmed.to_string());
                    }
                }
                return keys;
            }
        }
    }

    extract_unmanaged_agent_keys_from_headers(raw)
}

fn extract_unmanaged_agent_keys_from_headers(raw: &str) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    for line in raw.lines() {
        let Some(inner) = extract_table_header_inner(line) else {
            continue;
        };
        let Some(rest) = inner.strip_prefix("agents.") else {
            continue;
        };
        let rest = rest.trim();
        if rest.is_empty() {
            continue;
        }

        let key = if rest.starts_with('"') {
            parse_quoted_table_key(rest, '"')
        } else if rest.starts_with('\'') {
            parse_quoted_table_key(rest, '\'')
        } else {
            rest.split('.').next().map(|part| part.trim().to_string())
        };

        if let Some(key) = key.filter(|value| !value.is_empty()) {
            keys.insert(key);
        }
    }
    keys
}

fn parse_quoted_table_key(raw: &str, quote: char) -> Option<String> {
    let mut chars = raw.chars();
    if chars.next()? != quote {
        return None;
    }

    let mut value = String::new();
    let mut escaped = false;
    for ch in chars {
        if quote == '"' && escaped {
            value.push(ch);
            escaped = false;
            continue;
        }
        if quote == '"' && ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == quote {
            return Some(value);
        }
        value.push(ch);
    }

    None
}

fn extract_table_header_inner(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.starts_with("[[") || !trimmed.starts_with('[') {
        return None;
    }

    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut closing_index = None;
    for (index, ch) in trimmed.char_indices().skip(1) {
        match quote {
            Some('"') => {
                if escaped {
                    escaped = false;
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                    continue;
                }
                if ch == '"' {
                    quote = None;
                }
            }
            Some('\'') => {
                if ch == '\'' {
                    quote = None;
                }
            }
            None => {
                if ch == '"' || ch == '\'' {
                    quote = Some(ch);
                } else if ch == ']' {
                    closing_index = Some(index);
                    break;
                }
            }
            Some(_) => {}
        }
    }

    let end = closing_index?;
    let trailing = trimmed[end + 1..].trim();
    if !trailing.is_empty() && !trailing.starts_with('#') {
        return None;
    }

    Some(trimmed[1..end].trim())
}

fn strip_managed_blocks(current: &str, marker_pairs: &[(&str, &str)]) -> String {
    let mut normalized = current.replace("\r\n", "\n");
    loop {
        let mut changed = false;
        for &(begin_marker, end_marker) in marker_pairs {
            let next = strip_first_managed_block(&normalized, begin_marker, end_marker);
            if next != normalized {
                normalized = next;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    normalized
}

fn strip_first_managed_block(current: &str, begin_marker: &str, end_marker: &str) -> String {
    let normalized = current.replace("\r\n", "\n");
    let Some(begin_index) = normalized.find(begin_marker) else {
        return normalized;
    };
    let Some(end_index) = normalized[begin_index..].find(end_marker) else {
        return normalized;
    };
    let end_absolute = begin_index + end_index + end_marker.len();
    let prefix = normalized[..begin_index].trim_matches('\n');
    let suffix = normalized[end_absolute..].trim_matches('\n');
    match (prefix.is_empty(), suffix.is_empty()) {
        (true, true) => String::new(),
        (true, false) => format!("{suffix}\n"),
        (false, true) => format!("{prefix}\n"),
        (false, false) => format!("{prefix}\n\n{suffix}\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::{CodexSubagentConfigEntry, CodexSubagentRegistryWriter};

    fn count_occurrences(body: &str, needle: &str) -> usize {
        body.match_indices(needle).count()
    }

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

        let original_prompt = String::from("Use C:\\temp\\repo and regex \\d+\\.md");
        let writer = CodexSubagentRegistryWriter::new(home.clone());
        writer
            .write_managed_registries(&[CodexSubagentConfigEntry {
                scope: String::from("global"),
                workspace: None,
                subagent_key: String::from("reviewer"),
                description: String::from("Review code"),
                prompt: original_prompt.clone(),
                model: None,
            }])
            .expect("write registries");

        let subagent_cfg = home.join(".codex").join("agents").join("reviewer.toml");
        let raw = std::fs::read_to_string(&subagent_cfg).expect("read reviewer config");
        assert!(raw.contains("C:\\temp\\repo"));
        assert!(raw.contains("\\d+\\.md"));
        // Verify TOML roundtrip preserves backslashes
        let parsed: toml::Table = toml::from_str(&raw).expect("generated TOML must parse");
        let roundtripped = parsed
            .get("developer_instructions")
            .and_then(toml::Value::as_str)
            .expect("developer_instructions key");
        assert_eq!(roundtripped, original_prompt);
    }

    #[test]
    fn write_managed_registries_migrates_legacy_subagent_markers() {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("home");
        std::fs::create_dir_all(home.join(".codex")).expect("codex dir");
        let config_path = home.join(".codex").join("config.toml");
        std::fs::write(
            &config_path,
            "\
# skills-sync:subagents:begin
[agents.reviewer]
description = \"Legacy reviewer\"
config_file = \"agents/reviewer.toml\"
# skills-sync:subagents:end
",
        )
        .expect("write legacy config");

        let writer = CodexSubagentRegistryWriter::new(home.clone());
        writer
            .write_managed_registries(&[CodexSubagentConfigEntry {
                scope: String::from("global"),
                workspace: None,
                subagent_key: String::from("reviewer"),
                description: String::from("Review code"),
                prompt: String::from("Review instructions."),
                model: None,
            }])
            .expect("write registries");

        let raw = std::fs::read_to_string(config_path).expect("read config");
        assert!(raw.contains("# agent-sync:subagents:begin"));
        assert!(!raw.contains("# skills-sync:subagents:begin"));
        assert_eq!(count_occurrences(&raw, "[agents.reviewer]"), 1);
    }

    #[test]
    fn write_managed_registries_deduplicates_duplicate_input_entries_first_wins() {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("home");
        std::fs::create_dir_all(&home).expect("home");

        let writer = CodexSubagentRegistryWriter::new(home.clone());
        writer
            .write_managed_registries(&[
                CodexSubagentConfigEntry {
                    scope: String::from("global"),
                    workspace: None,
                    subagent_key: String::from("reviewer"),
                    description: String::from("First"),
                    prompt: String::from("First prompt"),
                    model: None,
                },
                CodexSubagentConfigEntry {
                    scope: String::from("global"),
                    workspace: None,
                    subagent_key: String::from("reviewer"),
                    description: String::from("Second"),
                    prompt: String::from("Second prompt"),
                    model: None,
                },
            ])
            .expect("write registries");

        let cfg = std::fs::read_to_string(home.join(".codex").join("config.toml")).expect("config");
        assert_eq!(count_occurrences(&cfg, "[agents.reviewer]"), 1);
        assert!(cfg.contains("description = \"First\""));
        assert!(!cfg.contains("description = \"Second\""));

        let subagent_cfg =
            std::fs::read_to_string(home.join(".codex").join("agents").join("reviewer.toml"))
                .expect("subagent file");
        assert!(subagent_cfg.contains("First prompt"));
        assert!(!subagent_cfg.contains("Second prompt"));
    }

    #[test]
    fn write_managed_registries_skips_existing_unmanaged_agent_table() {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("home");
        std::fs::create_dir_all(home.join(".codex")).expect("codex dir");
        let config_path = home.join(".codex").join("config.toml");
        std::fs::write(
            &config_path,
            "\
[agents.reviewer]
description = \"Manual reviewer\"
config_file = \"manual/reviewer.toml\"
",
        )
        .expect("write unmanaged table");

        let writer = CodexSubagentRegistryWriter::new(home.clone());
        writer
            .write_managed_registries(&[CodexSubagentConfigEntry {
                scope: String::from("global"),
                workspace: None,
                subagent_key: String::from("reviewer"),
                description: String::from("Managed reviewer"),
                prompt: String::from("Managed prompt"),
                model: None,
            }])
            .expect("write registries");

        let raw = std::fs::read_to_string(config_path).expect("read config");
        assert_eq!(count_occurrences(&raw, "[agents.reviewer]"), 1);
        assert!(raw.contains("Manual reviewer"));
        assert!(raw.contains("Skipped managed subagent 'reviewer'"));
        assert!(!home
            .join(".codex")
            .join("agents")
            .join("reviewer.toml")
            .exists());
    }

    #[test]
    fn write_managed_registries_skips_unmanaged_agent_table_with_inline_comment() {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("home");
        std::fs::create_dir_all(home.join(".codex")).expect("codex dir");
        let config_path = home.join(".codex").join("config.toml");
        std::fs::write(
            &config_path,
            "\
[agents.reviewer] # Manual reviewer override
description = \"Manual reviewer\"
config_file = \"manual/reviewer.toml\"
",
        )
        .expect("write unmanaged table");

        let writer = CodexSubagentRegistryWriter::new(home.clone());
        writer
            .write_managed_registries(&[CodexSubagentConfigEntry {
                scope: String::from("global"),
                workspace: None,
                subagent_key: String::from("reviewer"),
                description: String::from("Managed reviewer"),
                prompt: String::from("Managed prompt"),
                model: None,
            }])
            .expect("write registries");

        let raw = std::fs::read_to_string(config_path).expect("read config");
        assert_eq!(count_occurrences(&raw, "[agents.reviewer]"), 1);
        assert!(raw.contains("Manual reviewer"));
        assert!(raw.contains("Skipped managed subagent 'reviewer'"));
        assert!(!home
            .join(".codex")
            .join("agents")
            .join("reviewer.toml")
            .exists());
    }

    #[test]
    fn write_managed_registries_skips_unmanaged_agent_defined_with_dotted_keys() {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("home");
        std::fs::create_dir_all(home.join(".codex")).expect("codex dir");
        let config_path = home.join(".codex").join("config.toml");
        std::fs::write(
            &config_path,
            "\
agents.reviewer.description = \"Manual reviewer\"
agents.reviewer.config_file = \"manual/reviewer.toml\"
",
        )
        .expect("write dotted unmanaged agent");

        let writer = CodexSubagentRegistryWriter::new(home.clone());
        writer
            .write_managed_registries(&[CodexSubagentConfigEntry {
                scope: String::from("global"),
                workspace: None,
                subagent_key: String::from("reviewer"),
                description: String::from("Managed reviewer"),
                prompt: String::from("Managed prompt"),
                model: None,
            }])
            .expect("write registries");

        let raw = std::fs::read_to_string(config_path).expect("read config");
        assert!(raw.contains("agents.reviewer.description = \"Manual reviewer\""));
        assert!(raw.contains("agents.reviewer.config_file = \"manual/reviewer.toml\""));
        assert!(raw.contains("Skipped managed subagent 'reviewer'"));
        assert!(!raw.contains("[agents.reviewer]"));
        assert!(!home
            .join(".codex")
            .join("agents")
            .join("reviewer.toml")
            .exists());
    }

    #[test]
    fn write_managed_registries_skips_unmanaged_agent_defined_with_inline_table() {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("home");
        std::fs::create_dir_all(home.join(".codex")).expect("codex dir");
        let config_path = home.join(".codex").join("config.toml");
        std::fs::write(
            &config_path,
            "\
[agents]
reviewer = { description = \"Manual reviewer\", config_file = \"manual/reviewer.toml\" }
",
        )
        .expect("write inline-table unmanaged agent");

        let writer = CodexSubagentRegistryWriter::new(home.clone());
        writer
            .write_managed_registries(&[CodexSubagentConfigEntry {
                scope: String::from("global"),
                workspace: None,
                subagent_key: String::from("reviewer"),
                description: String::from("Managed reviewer"),
                prompt: String::from("Managed prompt"),
                model: None,
            }])
            .expect("write registries");

        let raw = std::fs::read_to_string(config_path).expect("read config");
        assert!(raw.contains(
            "reviewer = { description = \"Manual reviewer\", config_file = \"manual/reviewer.toml\" }"
        ));
        assert!(raw.contains("Skipped managed subagent 'reviewer'"));
        assert!(!raw.contains("[agents.reviewer]"));
        assert!(!home
            .join(".codex")
            .join("agents")
            .join("reviewer.toml")
            .exists());
    }

    #[test]
    fn write_managed_registries_collapses_multiple_managed_blocks_to_one() {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("home");
        std::fs::create_dir_all(home.join(".codex")).expect("codex dir");
        let config_path = home.join(".codex").join("config.toml");
        std::fs::write(
            &config_path,
            "\
# skills-sync:subagents:begin
[agents.old]
description = \"Old\"
config_file = \"agents/old.toml\"
# skills-sync:subagents:end

# agent-sync:subagents:begin
[agents.old]
description = \"Old duplicate\"
config_file = \"agents/old.toml\"
# agent-sync:subagents:end
",
        )
        .expect("write duplicate managed blocks");

        let writer = CodexSubagentRegistryWriter::new(home);
        writer
            .write_managed_registries(&[])
            .expect("write registries");

        let raw = std::fs::read_to_string(config_path).expect("read config");
        assert_eq!(count_occurrences(&raw, "# agent-sync:subagents:begin"), 1);
        assert_eq!(count_occurrences(&raw, "# agent-sync:subagents:end"), 1);
        assert!(!raw.contains("# skills-sync:subagents:begin"));
        assert!(raw.contains("# No managed subagent entries"));
    }
}
