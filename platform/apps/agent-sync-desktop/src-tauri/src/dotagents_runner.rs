use crate::dotagents_runtime::{DotagentsRuntimeManager, DotagentsRuntimeStatus};
use crate::settings::DotagentsScope;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DotagentsExecutionContext {
    pub scope: DotagentsScope,
    pub cwd: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DotagentsSkillListItem {
    pub name: String,
    pub source: String,
    pub status: DotagentsSkillStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wildcard: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DotagentsSkillStatus {
    Ok,
    Modified,
    Missing,
    Unlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DotagentsMcpListItem {
    pub name: String,
    pub transport: DotagentsMcpTransport,
    pub target: String,
    pub env: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DotagentsMcpTransport {
    Stdio,
    Http,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DotagentsCommandRequest {
    Install {
        frozen: bool,
    },
    Sync,
    SkillAdd {
        source: String,
        name: Option<String>,
        all: bool,
    },
    SkillRemove {
        name: String,
    },
    SkillUpdate {
        name: Option<String>,
    },
    McpAddStdio {
        name: String,
        command: String,
        args: Vec<String>,
        env: Vec<String>,
    },
    McpAddHttp {
        name: String,
        url: String,
        headers: Vec<String>,
        env: Vec<String>,
    },
    McpRemove {
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DotagentsCommandResult {
    pub success: bool,
    pub command: String,
    pub cwd: String,
    pub scope: DotagentsScope,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone)]
pub struct DotagentsRunner {
    runtime: DotagentsRuntimeManager,
    home_dir: PathBuf,
}

impl DotagentsRunner {
    pub fn new(home_dir: PathBuf, runtime: DotagentsRuntimeManager) -> Self {
        Self { runtime, home_dir }
    }

    fn dotagents_home(&self) -> PathBuf {
        self.home_dir.join(".agents")
    }

    pub fn runtime_status(&self) -> DotagentsRuntimeStatus {
        let error = self.runtime.check_pinned_cli_available().err();
        DotagentsRuntimeStatus {
            available: error.is_none(),
            expected_version: self.runtime.expected_version().to_string(),
            error,
        }
    }

    pub fn list_skills(
        &self,
        context: &DotagentsExecutionContext,
    ) -> Result<Vec<DotagentsSkillListItem>, String> {
        let raw =
            self.run_read_command(context, &[String::from("list"), String::from("--json")])?;
        let mut skills = parse_skill_list(&raw)?;
        let skills_dir = self.dotagents_home().join("skills");
        enrich_skill_descriptions(&mut skills, &skills_dir);
        Ok(skills)
    }

    pub fn list_mcp_servers(
        &self,
        context: &DotagentsExecutionContext,
    ) -> Result<Vec<DotagentsMcpListItem>, String> {
        let raw = self.run_read_command(
            context,
            &[
                String::from("mcp"),
                String::from("list"),
                String::from("--json"),
            ],
        )?;
        parse_mcp_list(&raw)
    }

    pub fn run_command(
        &self,
        context: &DotagentsExecutionContext,
        request: &DotagentsCommandRequest,
    ) -> Result<DotagentsCommandResult, String> {
        let args = build_command_args(request)?;
        let display_command = render_display_command(context.scope, &args);

        if let Err(error) = self.runtime.check_npx_available() {
            return Ok(preflight_failure_result(display_command, context, error));
        }

        self.execute_process(&args, context)
            .or_else(|error| Ok(preflight_failure_result(display_command, context, error)))
    }

    pub fn preflight_failure_result(
        &self,
        request: &DotagentsCommandRequest,
        context: &DotagentsExecutionContext,
        error: impl Into<String>,
    ) -> Result<DotagentsCommandResult, String> {
        let args = build_command_args(request)?;
        Ok(preflight_failure_result(
            render_display_command(context.scope, &args),
            context,
            error,
        ))
    }

    fn run_read_command(
        &self,
        context: &DotagentsExecutionContext,
        args: &[String],
    ) -> Result<String, String> {
        self.runtime.check_npx_available()?;
        let result = self.execute_process(args, context)?;
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
        context: &DotagentsExecutionContext,
    ) -> Result<DotagentsCommandResult, String> {
        let display_command = render_display_command(context.scope, args);
        let start = Instant::now();

        let mut command = Command::new("npx");
        command.arg("--yes");
        command.arg(self.runtime.npx_package_spec());

        if matches!(context.scope, DotagentsScope::User) {
            command.arg("--user");
        }

        for arg in args {
            command.arg(arg);
        }

        command.current_dir(&context.cwd);
        command.env("HOME", &self.home_dir);
        command.env("DOTAGENTS_HOME", self.dotagents_home());
        command.env("NO_COLOR", "1");
        command.env("DOTAGENTS_NO_COLOR", "1");

        let output = command
            .output()
            .map_err(|error| format!("failed to launch npx: {error}"))?;
        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(DotagentsCommandResult {
            success: output.status.success(),
            command: display_command,
            cwd: context.cwd.display().to_string(),
            scope: context.scope,
            exit_code: output.status.code(),
            duration_ms,
            stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

pub fn build_command_args(request: &DotagentsCommandRequest) -> Result<Vec<String>, String> {
    match request {
        DotagentsCommandRequest::Install { frozen } => {
            let mut args = vec![String::from("install")];
            if *frozen {
                args.push(String::from("--frozen"));
            }
            Ok(args)
        }
        DotagentsCommandRequest::Sync => Ok(vec![String::from("sync")]),
        DotagentsCommandRequest::SkillAdd { source, name, all } => {
            if source.trim().is_empty() {
                return Err(String::from("skill add requires a source"));
            }
            if *all && name.is_some() {
                return Err(String::from(
                    "skill add cannot use explicit name and wildcard mode together",
                ));
            }
            if !*all && name.as_deref().unwrap_or("").trim().is_empty() {
                return Err(String::from(
                    "skill add requires either an explicit name or wildcard mode",
                ));
            }

            let mut args = vec![String::from("add"), source.trim().to_string()];
            if *all {
                args.push(String::from("--all"));
            } else if let Some(name) = name {
                args.push(String::from("--name"));
                args.push(name.trim().to_string());
            }
            Ok(args)
        }
        DotagentsCommandRequest::SkillRemove { name } => {
            require_non_empty(name, "skill remove requires a skill name")?;
            Ok(vec![String::from("remove"), name.trim().to_string()])
        }
        DotagentsCommandRequest::SkillUpdate { name } => {
            let mut args = vec![String::from("update")];
            if let Some(name) = name {
                require_non_empty(name, "skill update requires a non-empty skill name")?;
                args.push(name.trim().to_string());
            }
            Ok(args)
        }
        DotagentsCommandRequest::McpAddStdio {
            name,
            command,
            args,
            env,
        } => {
            require_non_empty(name, "MCP add requires a server name")?;
            require_non_empty(command, "MCP stdio add requires a command")?;

            let mut built = vec![
                String::from("mcp"),
                String::from("add"),
                name.trim().to_string(),
                String::from("--command"),
                command.trim().to_string(),
            ];
            for value in args {
                require_non_empty(value, "MCP stdio args cannot be empty")?;
                built.push(String::from("--args"));
                built.push(value.trim().to_string());
            }
            for value in env {
                require_non_empty(value, "MCP env entries cannot be empty")?;
                built.push(String::from("--env"));
                built.push(value.trim().to_string());
            }
            Ok(built)
        }
        DotagentsCommandRequest::McpAddHttp {
            name,
            url,
            headers,
            env,
        } => {
            require_non_empty(name, "MCP add requires a server name")?;
            require_non_empty(url, "MCP HTTP add requires a URL")?;

            let mut built = vec![
                String::from("mcp"),
                String::from("add"),
                name.trim().to_string(),
                String::from("--url"),
                url.trim().to_string(),
            ];
            for value in headers {
                require_non_empty(value, "MCP header entries cannot be empty")?;
                built.push(String::from("--header"));
                built.push(value.trim().to_string());
            }
            for value in env {
                require_non_empty(value, "MCP env entries cannot be empty")?;
                built.push(String::from("--env"));
                built.push(value.trim().to_string());
            }
            Ok(built)
        }
        DotagentsCommandRequest::McpRemove { name } => {
            require_non_empty(name, "MCP remove requires a server name")?;
            Ok(vec![
                String::from("mcp"),
                String::from("remove"),
                name.trim().to_string(),
            ])
        }
    }
}

pub fn render_display_command(scope: DotagentsScope, args: &[String]) -> String {
    let mut pieces = vec![String::from("dotagents")];
    if matches!(scope, DotagentsScope::User) {
        pieces.push(String::from("--user"));
    }
    pieces.extend(args.iter().cloned());
    pieces.join(" ")
}

pub fn parse_skill_list(raw: &str) -> Result<Vec<DotagentsSkillListItem>, String> {
    parse_declared_list(raw, "No skills declared in agents.toml.", "skill")
}

pub fn parse_mcp_list(raw: &str) -> Result<Vec<DotagentsMcpListItem>, String> {
    parse_declared_list(raw, "No MCP servers declared in agents.toml.", "MCP")
}

fn parse_declared_list<T>(raw: &str, empty_message: &str, label: &str) -> Result<Vec<T>, String>
where
    T: for<'de> Deserialize<'de>,
{
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == empty_message {
        return Ok(Vec::new());
    }

    serde_json::from_str(trimmed)
        .map_err(|error| format!("failed to parse {label} list JSON: {error}"))
}

fn enrich_skill_descriptions(skills: &mut [DotagentsSkillListItem], skills_dir: &Path) {
    for skill in skills.iter_mut() {
        if skill.description.is_some() {
            continue;
        }
        let skill_md = skills_dir.join(&skill.name).join("SKILL.md");
        if let Some(desc) = read_skill_description(&skill_md) {
            skill.description = Some(desc);
        }
    }
}

fn read_skill_description(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }
    let after_open = &trimmed[3..];
    let close_pos = after_open.find("---")?;
    let frontmatter = &after_open[..close_pos];
    for line in frontmatter.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("description:") {
            let value = rest.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn combine_output(primary: &str, secondary: &str) -> String {
    match (primary.trim().is_empty(), secondary.trim().is_empty()) {
        (true, true) => String::new(),
        (false, true) => primary.trim().to_string(),
        (true, false) => secondary.trim().to_string(),
        (false, false) => format!("{}\n{}", primary.trim(), secondary.trim()),
    }
}

fn preflight_failure_result(
    display_command: String,
    context: &DotagentsExecutionContext,
    error: impl Into<String>,
) -> DotagentsCommandResult {
    DotagentsCommandResult {
        success: false,
        command: display_command,
        cwd: context.cwd.display().to_string(),
        scope: context.scope,
        exit_code: None,
        duration_ms: 0,
        stdout: String::new(),
        stderr: error.into(),
    }
}

fn require_non_empty(value: &str, message: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(message.to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        build_command_args, parse_mcp_list, parse_skill_list, render_display_command,
        DotagentsCommandRequest, DotagentsRunner,
    };
    use crate::dotagents_runtime::DotagentsRuntimeManager;
    use crate::settings::DotagentsScope;
    use std::path::PathBuf;

    #[test]
    fn scope_to_argv_inserts_user_flag_only_for_user_scope() {
        let args = vec![String::from("list"), String::from("--json")];
        assert_eq!(
            render_display_command(DotagentsScope::User, &args),
            "dotagents --user list --json"
        );
        assert_eq!(
            render_display_command(DotagentsScope::Project, &args),
            "dotagents list --json"
        );
    }

    #[test]
    fn parse_vendor_skill_list_json() {
        let parsed = parse_skill_list(
            r#"[{"name":"lint","source":"owner/repo","status":"ok","commit":"deadbeef","wildcard":"owner/repo"}]"#,
        )
        .expect("parse skill list");
        assert_eq!(parsed[0].name, "lint");
        assert_eq!(parsed[0].wildcard.as_deref(), Some("owner/repo"));
    }

    #[test]
    fn parse_vendor_mcp_list_json() {
        let parsed = parse_mcp_list(
            r#"[{"name":"github","transport":"stdio","target":"npx","env":["GITHUB_TOKEN"]}]"#,
        )
        .expect("parse MCP list");
        assert_eq!(parsed[0].name, "github");
        assert_eq!(parsed[0].env, vec![String::from("GITHUB_TOKEN")]);
    }

    #[test]
    fn builds_supported_command_variants() {
        assert_eq!(
            build_command_args(&DotagentsCommandRequest::SkillAdd {
                source: String::from("owner/repo"),
                name: Some(String::from("lint")),
                all: false,
            })
            .expect("skill add explicit"),
            vec!["add", "owner/repo", "--name", "lint"]
        );
        assert_eq!(
            build_command_args(&DotagentsCommandRequest::SkillAdd {
                source: String::from("owner/repo"),
                name: None,
                all: true,
            })
            .expect("skill add wildcard"),
            vec!["add", "owner/repo", "--all"]
        );
        assert_eq!(
            build_command_args(&DotagentsCommandRequest::Install { frozen: true })
                .expect("install frozen"),
            vec!["install", "--frozen"]
        );
        assert_eq!(
            build_command_args(&DotagentsCommandRequest::SkillUpdate {
                name: Some(String::from("lint")),
            })
            .expect("skill update"),
            vec!["update", "lint"]
        );
        assert_eq!(
            build_command_args(&DotagentsCommandRequest::McpAddStdio {
                name: String::from("github"),
                command: String::from("npx"),
                args: vec![
                    String::from("-y"),
                    String::from("@modelcontextprotocol/server-github")
                ],
                env: vec![String::from("GITHUB_TOKEN")],
            })
            .expect("mcp add stdio"),
            vec![
                "mcp",
                "add",
                "github",
                "--command",
                "npx",
                "--args",
                "-y",
                "--args",
                "@modelcontextprotocol/server-github",
                "--env",
                "GITHUB_TOKEN"
            ]
        );
        assert_eq!(
            build_command_args(&DotagentsCommandRequest::McpAddHttp {
                name: String::from("remote"),
                url: String::from("https://mcp.example.com/sse"),
                headers: vec![String::from("Authorization:Bearer token")],
                env: vec![String::from("API_TOKEN")],
            })
            .expect("mcp add http"),
            vec![
                "mcp",
                "add",
                "remote",
                "--url",
                "https://mcp.example.com/sse",
                "--header",
                "Authorization:Bearer token",
                "--env",
                "API_TOKEN"
            ]
        );
        assert_eq!(
            build_command_args(&DotagentsCommandRequest::McpRemove {
                name: String::from("remote"),
            })
            .expect("mcp remove"),
            vec!["mcp", "remove", "remote"]
        );
    }

    #[test]
    fn skill_add_requires_name_or_wildcard() {
        let error = build_command_args(&DotagentsCommandRequest::SkillAdd {
            source: String::from("owner/repo"),
            name: None,
            all: false,
        })
        .expect_err("missing selector");
        assert!(error.contains("requires either an explicit name or wildcard mode"));
    }

    #[test]
    fn dotagents_home_is_always_derived_from_user_home() {
        let runner = DotagentsRunner::new(
            PathBuf::from("/tmp/home"),
            DotagentsRuntimeManager::new(),
        );

        assert_eq!(runner.dotagents_home(), PathBuf::from("/tmp/home/.agents"));
    }
}
