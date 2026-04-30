use crate::cli_util::{combine_output, read_skill_description, require_non_empty};
use crate::skills_runtime::{SkillsRuntimeManager, SkillsRuntimeStatus};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SkillsCliScope {
    #[default]
    Global,
    Project,
}

impl SkillsCliScope {
    pub fn flag(self) -> &'static str {
        match self {
            SkillsCliScope::Global => "-g",
            SkillsCliScope::Project => "-p",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillsExecutionContext {
    pub scope: SkillsCliScope,
    pub cwd: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsCliListItem {
    pub name: String,
    pub path: String,
    pub scope: SkillsCliScope,
    pub agents: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SkillsCliCommandRequest {
    Add {
        source: String,
        agents: Vec<String>,
        scope: SkillsCliScope,
    },
    Remove {
        name: String,
        agents: Vec<String>,
        scope: SkillsCliScope,
    },
    Update {
        names: Vec<String>,
        scope: SkillsCliScope,
    },
    RestoreLock {
        scope: SkillsCliScope,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsCliCommandResult {
    pub success: bool,
    pub command: String,
    pub cwd: String,
    pub scope: SkillsCliScope,
    pub agents: Vec<String>,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone)]
pub struct SkillsRunner {
    runtime: SkillsRuntimeManager,
    home_dir: PathBuf,
}

impl SkillsRunner {
    pub fn new(home_dir: PathBuf, runtime: SkillsRuntimeManager) -> Self {
        Self { runtime, home_dir }
    }

    pub fn runtime_status(&self) -> SkillsRuntimeStatus {
        let error = self.runtime.check_pinned_cli_available().err();
        SkillsRuntimeStatus {
            available: error.is_none(),
            expected_version: self.runtime.expected_version().to_string(),
            error,
        }
    }

    pub fn list_skills(
        &self,
        context: &SkillsExecutionContext,
    ) -> Result<Vec<SkillsCliListItem>, String> {
        let args = vec![
            String::from("list"),
            String::from("--json"),
            context.scope.flag().to_string(),
        ];
        let raw = self.run_read_command(context, &args)?;
        let mut skills = parse_list_json(&raw)?;
        let lock_entries = self.read_lock_for_scope(context);
        enrich_skills(&mut skills, &lock_entries);
        Ok(skills)
    }

    pub fn run_command(
        &self,
        context: &SkillsExecutionContext,
        request: &SkillsCliCommandRequest,
    ) -> Result<SkillsCliCommandResult, String> {
        let args = build_command_args(request)?;
        let agents = command_agents(request);
        let display_command = render_display_command(&args);

        if let Err(error) = self.runtime.check_npx_available() {
            return Ok(preflight_failure_result(
                display_command,
                context,
                agents,
                error,
            ));
        }

        self.execute_process(&args, context, agents.clone())
            .or_else(|error| {
                Ok(preflight_failure_result(
                    render_display_command(&args),
                    context,
                    agents,
                    error,
                ))
            })
    }

    pub fn detect_installed_agents(&self) -> Vec<String> {
        let mut found = Vec::new();
        for (display, dirs) in installed_agent_directories() {
            for rel in *dirs {
                let path = self.home_dir.join(rel);
                if path.exists() {
                    found.push(display.to_string());
                    break;
                }
            }
        }
        found
    }

    fn run_read_command(
        &self,
        context: &SkillsExecutionContext,
        args: &[String],
    ) -> Result<String, String> {
        self.runtime.check_npx_available()?;
        let result = self.execute_process(args, context, Vec::new())?;
        if result.success {
            return Ok(result.stdout);
        }

        Err(format!(
            "{} failed with exit code {:?}: {}",
            result.command,
            result.exit_code,
            combine_output(&result.stderr, &result.stdout)
        ))
    }

    fn execute_process(
        &self,
        args: &[String],
        context: &SkillsExecutionContext,
        agents: Vec<String>,
    ) -> Result<SkillsCliCommandResult, String> {
        let display_command = render_display_command(args);
        let start = Instant::now();

        let mut command = Command::new("npx");
        command.arg("-y");
        command.arg(self.runtime.npx_package_spec());

        for arg in args {
            command.arg(arg);
        }

        command.current_dir(&context.cwd);
        command.env("HOME", &self.home_dir);
        command.env("NO_COLOR", "1");

        let output = command
            .output()
            .map_err(|error| format!("failed to launch npx: {error}"))?;
        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(SkillsCliCommandResult {
            success: output.status.success(),
            command: display_command,
            cwd: context.cwd.display().to_string(),
            scope: context.scope,
            agents,
            exit_code: output.status.code(),
            duration_ms,
            stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }

    fn read_lock_for_scope(&self, context: &SkillsExecutionContext) -> HashMap<String, LockEntry> {
        let lock_path = match context.scope {
            SkillsCliScope::Project => context.cwd.join("skills-lock.json"),
            SkillsCliScope::Global => self.home_dir.join(".agents").join("skills-lock.json"),
        };
        read_lock_file(&lock_path).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Default)]
struct LockEntry {
    source: Option<String>,
    version: Option<String>,
}

pub fn build_command_args(request: &SkillsCliCommandRequest) -> Result<Vec<String>, String> {
    match request {
        SkillsCliCommandRequest::Add {
            source,
            agents,
            scope,
        } => {
            require_non_empty(source, "skills add requires a source")?;
            require_at_least_one_agent(agents)?;
            let mut args = vec![
                String::from("add"),
                source.trim().to_string(),
                scope.flag().to_string(),
            ];
            args.push(String::from("--agent"));
            for agent in agents {
                args.push(agent_kebab(agent));
            }
            Ok(args)
        }
        SkillsCliCommandRequest::Remove {
            name,
            agents,
            scope,
        } => {
            require_non_empty(name, "skills remove requires a skill name")?;
            require_at_least_one_agent(agents)?;
            let mut args = vec![
                String::from("remove"),
                name.trim().to_string(),
                scope.flag().to_string(),
            ];
            args.push(String::from("--agent"));
            for agent in agents {
                args.push(agent_kebab(agent));
            }
            Ok(args)
        }
        SkillsCliCommandRequest::Update { names, scope } => {
            let mut args = vec![String::from("update")];
            for name in names {
                require_non_empty(name, "skill update requires non-empty names")?;
                args.push(name.trim().to_string());
            }
            args.push(scope.flag().to_string());
            Ok(args)
        }
        SkillsCliCommandRequest::RestoreLock { scope } => {
            Ok(vec![String::from("restore"), scope.flag().to_string()])
        }
    }
}

fn command_agents(request: &SkillsCliCommandRequest) -> Vec<String> {
    match request {
        SkillsCliCommandRequest::Add { agents, .. }
        | SkillsCliCommandRequest::Remove { agents, .. } => agents.clone(),
        SkillsCliCommandRequest::Update { .. } | SkillsCliCommandRequest::RestoreLock { .. } => {
            Vec::new()
        }
    }
}

pub fn render_display_command(args: &[String]) -> String {
    let mut pieces = vec![String::from("skills")];
    pieces.extend(args.iter().cloned());
    pieces.join(" ")
}

/// Convert a display name (e.g. "Claude Code") to the kebab-case slug
/// the Skills CLI expects after `--agent` (e.g. "claude-code").
pub fn agent_kebab(display: &str) -> String {
    if let Some(canonical) = display_to_kebab_map().get(display) {
        return canonical.to_string();
    }
    display
        .trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
}

/// Inverse of [`agent_kebab`] — turn a kebab-case slug back into its
/// canonical display name. Falls back to title-casing word-by-word.
#[allow(dead_code)]
pub fn agent_display(kebab: &str) -> String {
    if let Some(display) = kebab_to_display_map().get(kebab) {
        return display.to_string();
    }
    kebab
        .split('-')
        .filter(|s| !s.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn display_to_kebab_map() -> BTreeMap<&'static str, &'static str> {
    let mut map = BTreeMap::new();
    for (display, kebab) in agent_aliases() {
        map.insert(*display, *kebab);
    }
    map
}

#[allow(dead_code)]
fn kebab_to_display_map() -> BTreeMap<&'static str, &'static str> {
    let mut map = BTreeMap::new();
    for (display, kebab) in agent_aliases() {
        map.insert(*kebab, *display);
    }
    map
}

fn agent_aliases() -> &'static [(&'static str, &'static str)] {
    &[
        ("Claude Code", "claude-code"),
        ("Cursor", "cursor"),
        ("Codex", "codex"),
        ("Cline", "cline"),
        ("Windsurf", "windsurf"),
        ("Continue", "continue"),
        ("Aider", "aider"),
        ("Roo Code", "roo-code"),
    ]
}

fn installed_agent_directories() -> &'static [(&'static str, &'static [&'static str])] {
    &[
        ("Claude Code", &[".claude"]),
        ("Cursor", &[".cursor", ".config/Cursor"]),
        ("Codex", &[".codex"]),
        ("Cline", &[".cline"]),
        ("Windsurf", &[".windsurf", ".config/Windsurf"]),
        ("Continue", &[".continue"]),
        ("Aider", &[".aider"]),
    ]
}

pub fn parse_list_json(raw: &str) -> Result<Vec<SkillsCliListItem>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(trimmed)
        .map_err(|error| format!("failed to parse skills list JSON: {error}"))
}

fn enrich_skills(skills: &mut [SkillsCliListItem], lock: &HashMap<String, LockEntry>) {
    for skill in skills.iter_mut() {
        if let Some(entry) = lock.get(&skill.name) {
            if skill.source.is_none() {
                skill.source.clone_from(&entry.source);
            }
            if skill.version.is_none() {
                skill.version.clone_from(&entry.version);
            }
        }
        if skill.description.is_none() {
            let skill_md = Path::new(&skill.path).join("SKILL.md");
            skill.description = read_skill_description(&skill_md);
        }
    }
}

fn read_lock_file(path: &Path) -> Option<HashMap<String, LockEntry>> {
    let content = fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&content).ok()?;
    let mut map = HashMap::new();

    // Support common shapes:
    //   { "skills": { "<name>": { "source": "...", "version": "..." } } }
    //   { "skills": [ { "name": "...", "source": "...", "version": "..." } ] }
    if let Some(skills) = value.get("skills") {
        if let Some(obj) = skills.as_object() {
            for (name, entry) in obj {
                map.insert(name.clone(), lock_entry_from_value(entry));
            }
        } else if let Some(arr) = skills.as_array() {
            for entry in arr {
                if let Some(name) = entry.get("name").and_then(|v| v.as_str()) {
                    map.insert(name.to_string(), lock_entry_from_value(entry));
                }
            }
        }
    } else if let Some(obj) = value.as_object() {
        // Bare object: { "<name>": { ... } }
        for (name, entry) in obj {
            map.insert(name.clone(), lock_entry_from_value(entry));
        }
    }

    Some(map)
}

fn lock_entry_from_value(value: &serde_json::Value) -> LockEntry {
    let source = value
        .get("source")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let version = value
        .get("version")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    LockEntry { source, version }
}

fn preflight_failure_result(
    display_command: String,
    context: &SkillsExecutionContext,
    agents: Vec<String>,
    error: impl Into<String>,
) -> SkillsCliCommandResult {
    SkillsCliCommandResult {
        success: false,
        command: display_command,
        cwd: context.cwd.display().to_string(),
        scope: context.scope,
        agents,
        exit_code: None,
        duration_ms: 0,
        stdout: String::new(),
        stderr: error.into(),
    }
}

fn require_at_least_one_agent(agents: &[String]) -> Result<(), String> {
    if agents.iter().all(|a| a.trim().is_empty()) {
        return Err(String::from(
            "skills command requires at least one target agent",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        agent_display, agent_kebab, build_command_args, parse_list_json, SkillsCliCommandRequest,
        SkillsCliScope,
    };

    #[test]
    fn parses_list_json_into_items() {
        let raw = r#"[
            {
                "name": "adapt",
                "path": "/Users/me/.claude/skills/adapt",
                "scope": "global",
                "agents": ["Claude Code", "Cline"]
            }
        ]"#;
        let parsed = parse_list_json(raw).expect("parse");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "adapt");
        assert_eq!(parsed[0].agents, vec!["Claude Code", "Cline"]);
        assert_eq!(parsed[0].scope, SkillsCliScope::Global);
    }

    #[test]
    fn empty_list_input_returns_empty() {
        assert!(parse_list_json("").expect("empty").is_empty());
        assert!(parse_list_json("   \n").expect("ws").is_empty());
    }

    #[test]
    fn agent_kebab_uses_known_aliases() {
        assert_eq!(agent_kebab("Claude Code"), "claude-code");
        assert_eq!(agent_kebab("Cursor"), "cursor");
        assert_eq!(agent_kebab("Roo Code"), "roo-code");
    }

    #[test]
    fn agent_kebab_falls_back_to_lowercase_hyphenation() {
        assert_eq!(agent_kebab("Brand New Agent"), "brand-new-agent");
    }

    #[test]
    fn agent_display_inverse_for_known_aliases() {
        assert_eq!(agent_display("claude-code"), "Claude Code");
        assert_eq!(agent_display("windsurf"), "Windsurf");
    }

    #[test]
    fn agent_display_falls_back_to_title_case() {
        assert_eq!(agent_display("brand-new-agent"), "Brand New Agent");
    }

    #[test]
    fn builds_add_command_args_in_kebab_case() {
        let args = build_command_args(&SkillsCliCommandRequest::Add {
            source: String::from("vercel-labs/agent-skills"),
            agents: vec![String::from("Claude Code"), String::from("Cursor")],
            scope: SkillsCliScope::Global,
        })
        .expect("add");
        assert_eq!(
            args,
            vec![
                "add",
                "vercel-labs/agent-skills",
                "-g",
                "--agent",
                "claude-code",
                "cursor",
            ]
        );
    }

    #[test]
    fn builds_remove_command_with_project_scope() {
        let args = build_command_args(&SkillsCliCommandRequest::Remove {
            name: String::from("adapt"),
            agents: vec![String::from("Claude Code")],
            scope: SkillsCliScope::Project,
        })
        .expect("remove");
        assert_eq!(
            args,
            vec!["remove", "adapt", "-p", "--agent", "claude-code"]
        );
    }

    #[test]
    fn builds_update_command_for_subset_of_skills() {
        let args = build_command_args(&SkillsCliCommandRequest::Update {
            names: vec![String::from("adapt"), String::from("polish")],
            scope: SkillsCliScope::Global,
        })
        .expect("update");
        assert_eq!(args, vec!["update", "adapt", "polish", "-g"]);
    }

    #[test]
    fn builds_update_command_for_all_when_names_empty() {
        let args = build_command_args(&SkillsCliCommandRequest::Update {
            names: vec![],
            scope: SkillsCliScope::Project,
        })
        .expect("update all");
        assert_eq!(args, vec!["update", "-p"]);
    }

    #[test]
    fn add_requires_at_least_one_agent() {
        let err = build_command_args(&SkillsCliCommandRequest::Add {
            source: String::from("owner/repo"),
            agents: vec![],
            scope: SkillsCliScope::Global,
        })
        .expect_err("missing agents");
        assert!(err.contains("at least one target agent"));
    }

    #[test]
    fn add_requires_non_empty_source() {
        let err = build_command_args(&SkillsCliCommandRequest::Add {
            source: String::from("   "),
            agents: vec![String::from("Claude Code")],
            scope: SkillsCliScope::Global,
        })
        .expect_err("missing source");
        assert!(err.contains("source"));
    }

    #[test]
    fn restore_lock_command_uses_scope_flag() {
        let args = build_command_args(&SkillsCliCommandRequest::RestoreLock {
            scope: SkillsCliScope::Project,
        })
        .expect("restore");
        assert_eq!(args, vec!["restore", "-p"]);
    }
}
