use crate::dotagents_runtime::{DotagentsRuntimeManager, DotagentsRuntimeStatus};
use crate::settings::DotagentsScope;
use serde::{Deserialize, Serialize};
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

    pub fn runtime_status(&self) -> DotagentsRuntimeStatus {
        let binary = match self.runtime.resolve_binary() {
            Ok(binary) => binary,
            Err(error) => {
                return DotagentsRuntimeStatus {
                    available: false,
                    expected_version: self.runtime.expected_version().to_string(),
                    actual_version: None,
                    binary_path: None,
                    error: Some(error),
                };
            }
        };

        if let Err(error) = self.runtime.verify_checksum(&binary) {
            return DotagentsRuntimeStatus {
                available: false,
                expected_version: self.runtime.expected_version().to_string(),
                actual_version: None,
                binary_path: Some(binary.path.display().to_string()),
                error: Some(error),
            };
        }

        let version_result = self.execute_process(
            &binary.path,
            &[],
            &DotagentsExecutionContext {
                scope: DotagentsScope::User,
                cwd: self.home_dir.clone(),
            },
            Some("--version"),
        );
        let combined = match version_result {
            Ok(result) => combine_output(&result.stdout, &result.stderr),
            Err(error) => {
                return DotagentsRuntimeStatus {
                    available: false,
                    expected_version: self.runtime.expected_version().to_string(),
                    actual_version: None,
                    binary_path: Some(binary.path.display().to_string()),
                    error: Some(error),
                };
            }
        };

        match self.runtime.parse_version_output(&combined) {
            Ok(actual_version) => DotagentsRuntimeStatus {
                available: true,
                expected_version: self.runtime.expected_version().to_string(),
                actual_version: Some(actual_version),
                binary_path: Some(binary.path.display().to_string()),
                error: None,
            },
            Err(error) => DotagentsRuntimeStatus {
                available: false,
                expected_version: self.runtime.expected_version().to_string(),
                actual_version: None,
                binary_path: Some(binary.path.display().to_string()),
                error: Some(error),
            },
        }
    }

    pub fn list_skills(
        &self,
        context: &DotagentsExecutionContext,
    ) -> Result<Vec<DotagentsSkillListItem>, String> {
        let raw =
            self.run_read_command(context, &[String::from("list"), String::from("--json")])?;
        parse_skill_list(&raw)
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
        let binary = match self.runtime.resolve_binary() {
            Ok(binary) => binary,
            Err(error) => {
                return Ok(preflight_failure_result(display_command, context, error));
            }
        };

        if let Err(error) = self.runtime.verify_checksum(&binary) {
            return Ok(preflight_failure_result(display_command, context, error));
        }

        match self.execute_process(&binary.path, &args, context, None) {
            Ok(result) => Ok(result),
            Err(error) => Ok(preflight_failure_result(display_command, context, error)),
        }
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
        let binary = self.runtime.resolve_binary()?;
        self.runtime.verify_checksum(&binary)?;
        let result = self.execute_process(&binary.path, args, context, None)?;
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
        binary_path: &Path,
        args: &[String],
        context: &DotagentsExecutionContext,
        command_override: Option<&str>,
    ) -> Result<DotagentsCommandResult, String> {
        let display_command = command_override
            .map(str::to_string)
            .unwrap_or_else(|| render_display_command(context.scope, args));
        let start = Instant::now();

        let mut rendered_args = Vec::new();
        let mut command = build_process_command(binary_path, context.scope, &mut rendered_args);
        command.current_dir(&context.cwd);
        command.env("HOME", &self.home_dir);
        command.env("DOTAGENTS_HOME", self.home_dir.join(".agents"));
        command.env("NO_COLOR", "1");
        command.env("DOTAGENTS_NO_COLOR", "1");

        for arg in args {
            command.arg(arg);
            rendered_args.push(arg.clone());
        }

        let output = command
            .output()
            .map_err(|error| format!("failed to launch {}: {error}", binary_path.display()))?;
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
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "No skills declared in agents.toml." {
        return Ok(Vec::new());
    }

    serde_json::from_str(trimmed)
        .map_err(|error| format!("failed to parse skill list JSON: {error}"))
}

pub fn parse_mcp_list(raw: &str) -> Result<Vec<DotagentsMcpListItem>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "No MCP servers declared in agents.toml." {
        return Ok(Vec::new());
    }

    serde_json::from_str(trimmed).map_err(|error| format!("failed to parse MCP list JSON: {error}"))
}

fn build_process_command(
    binary_path: &Path,
    scope: DotagentsScope,
    rendered_args: &mut Vec<String>,
) -> Command {
    #[cfg(windows)]
    {
        if is_windows_shell_script(binary_path) {
            let shell = std::env::var("COMSPEC").unwrap_or_else(|_| String::from("cmd.exe"));
            let mut command = Command::new(&shell);
            command.arg("/C");
            command.arg(binary_path);
            rendered_args.push(shell);
            rendered_args.push(String::from("/C"));
            rendered_args.push(binary_path.display().to_string());
            if matches!(scope, DotagentsScope::User) {
                command.arg("--user");
                rendered_args.push(String::from("--user"));
            }
            return command;
        }
    }

    let mut command = Command::new(binary_path);
    rendered_args.push(binary_path.display().to_string());
    if matches!(scope, DotagentsScope::User) {
        command.arg("--user");
        rendered_args.push(String::from("--user"));
    }
    command
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

#[cfg(windows)]
fn is_windows_shell_script(binary_path: &Path) -> bool {
    binary_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("cmd") || ext.eq_ignore_ascii_case("bat"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::{
        build_command_args, parse_mcp_list, parse_skill_list, render_display_command,
        DotagentsCommandRequest, DotagentsExecutionContext, DotagentsRunner,
    };
    use crate::dotagents_runtime::DotagentsRuntimeManager;
    use crate::settings::DotagentsScope;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

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
    #[cfg(unix)]
    fn captures_success_and_failure_transcripts() {
        let temp = tempdir().expect("tempdir");
        let script_path = temp.path().join("dotagents");
        fs::write(
            &script_path,
            r#"#!/bin/sh
if [ "$1" = "--user" ]; then
  shift
fi
if [ "$1" = "--version" ]; then
  echo "0.10.0"
  exit 0
fi
if [ "$1" = "sync" ]; then
  echo "synced ok"
  exit 0
fi
echo "boom" >&2
exit 14
"#,
        )
        .expect("write script");
        let mut perms = fs::metadata(&script_path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).expect("chmod");

        let runner = DotagentsRunner::new(
            temp.path().to_path_buf(),
            DotagentsRuntimeManager::new().with_override_binary(script_path),
        );
        let context = DotagentsExecutionContext {
            scope: DotagentsScope::User,
            cwd: temp.path().to_path_buf(),
        };

        let success = runner
            .run_command(&context, &DotagentsCommandRequest::Sync)
            .expect("run success");
        assert!(success.success);
        assert_eq!(success.exit_code, Some(0));
        assert_eq!(success.stdout, "synced ok");

        let failure = runner
            .run_command(
                &context,
                &DotagentsCommandRequest::SkillRemove {
                    name: String::from("missing"),
                },
            )
            .expect("run failure");
        assert!(!failure.success);
        assert_eq!(failure.exit_code, Some(14));
        assert_eq!(failure.stderr, "boom");
    }
}
