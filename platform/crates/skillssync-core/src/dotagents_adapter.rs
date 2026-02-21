use crate::dotagents_runtime::{DotagentsResolvedBinary, DotagentsRuntimeManager};
use crate::error::SyncEngineError;
use serde_json::Value as JsonValue;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct DotagentsCommandOutput {
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone)]
pub struct DotagentsAdapter {
    runtime: DotagentsRuntimeManager,
}

impl DotagentsAdapter {
    pub fn new(runtime: DotagentsRuntimeManager) -> Self {
        Self { runtime }
    }

    pub fn ensure_available(&self) -> Result<DotagentsResolvedBinary, SyncEngineError> {
        let binary = self.runtime.resolve_binary()?;
        self.runtime.verify_checksum(&binary)?;

        let version_output = self.execute_raw(
            &binary,
            &["--version"],
            self.runtime.home_directory(),
            false,
        )?;
        let combined = if version_output.stderr.trim().is_empty() {
            version_output.stdout.clone()
        } else if version_output.stdout.trim().is_empty() {
            version_output.stderr.clone()
        } else {
            format!("{}\n{}", version_output.stdout, version_output.stderr)
        };
        self.runtime.verify_version_output(&combined)?;

        Ok(binary)
    }

    pub fn run(
        &self,
        args: &[&str],
        cwd: &Path,
        user_scope: bool,
    ) -> Result<DotagentsCommandOutput, SyncEngineError> {
        let binary = self.ensure_available()?;
        self.execute_raw(&binary, args, cwd, user_scope)
    }

    pub fn run_json(
        &self,
        args: &[&str],
        cwd: &Path,
        user_scope: bool,
    ) -> Result<JsonValue, SyncEngineError> {
        let output = self.run(args, cwd, user_scope)?;
        serde_json::from_str(&output.stdout).map_err(SyncEngineError::Json)
    }

    fn execute_raw(
        &self,
        binary: &DotagentsResolvedBinary,
        args: &[&str],
        cwd: &Path,
        user_scope: bool,
    ) -> Result<DotagentsCommandOutput, SyncEngineError> {
        let mut rendered_command: Vec<String>;
        let mut command: Command;

        #[cfg(windows)]
        {
            if is_windows_shell_script(&binary.path) {
                rendered_command = vec![
                    String::from("cmd.exe"),
                    String::from("/C"),
                    binary.path.display().to_string(),
                ];
                command = Command::new("cmd.exe");
                command.arg("/C");
                command.arg(&binary.path);
            } else {
                rendered_command = vec![binary.path.display().to_string()];
                command = Command::new(&binary.path);
            }
        }

        #[cfg(not(windows))]
        {
            rendered_command = vec![binary.path.display().to_string()];
            command = Command::new(&binary.path);
        }

        command.current_dir(cwd);
        command.env("HOME", self.runtime.home_directory());
        command.env("NO_COLOR", "1");
        command.env("DOTAGENTS_NO_COLOR", "1");

        if user_scope {
            command.arg("--user");
            rendered_command.push(String::from("--user"));
        }

        for arg in args {
            command.arg(arg);
            rendered_command.push((*arg).to_string());
        }

        let output = command
            .output()
            .map_err(|error| SyncEngineError::io(&binary.path, error))?;
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        if output.status.success() {
            return Ok(DotagentsCommandOutput { stdout, stderr });
        }

        Err(SyncEngineError::DotagentsCommandFailed {
            command: rendered_command.join(" "),
            exit_code: output.status.code(),
            stderr,
            stdout,
        })
    }
}

#[cfg(any(windows, test))]
fn is_windows_shell_script(binary_path: &Path) -> bool {
    binary_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("cmd") || ext.eq_ignore_ascii_case("bat"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::{is_windows_shell_script, DotagentsAdapter};
    use crate::dotagents_runtime::DotagentsRuntimeManager;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use tempfile::TempDir;

    #[test]
    fn detects_windows_shell_script_extensions() {
        assert!(is_windows_shell_script(Path::new("dotagents.cmd")));
        assert!(is_windows_shell_script(Path::new("dotagents.CMD")));
        assert!(is_windows_shell_script(Path::new("dotagents.bat")));
        assert!(!is_windows_shell_script(Path::new("dotagents.exe")));
        assert!(!is_windows_shell_script(Path::new("dotagents")));
    }

    #[test]
    #[cfg(unix)]
    fn run_json_uses_user_scope_prefix() {
        let temp = TempDir::new().expect("tempdir");
        let script_path = temp.path().join("dotagents");
        fs::write(
            &script_path,
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "dotagents 0.10.0"
  exit 0
fi
if [ "$1" = "--user" ]; then
  shift
  if [ "$1" = "list" ] && [ "$2" = "--json" ]; then
    echo '[{"skill_key":"user-alpha","name":"User Alpha"}]'
    exit 0
  fi
fi
echo "unexpected args: $*" >&2
exit 9
"#,
        )
        .expect("write script");
        let mut perms = fs::metadata(&script_path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).expect("chmod");

        let runtime = DotagentsRuntimeManager::new(temp.path().to_path_buf())
            .with_override_binary(script_path);
        let adapter = DotagentsAdapter::new(runtime);
        let value = adapter
            .run_json(&["list", "--json"], temp.path(), true)
            .expect("run list json");

        assert_eq!(value.as_array().expect("array").len(), 1);
        assert_eq!(
            value[0]["skill_key"].as_str().expect("skill key"),
            "user-alpha"
        );
    }

    #[test]
    #[cfg(unix)]
    fn run_reports_stderr_on_failure() {
        let temp = TempDir::new().expect("tempdir");
        let script_path = temp.path().join("dotagents");
        fs::write(
            &script_path,
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "dotagents 0.10.0"
  exit 0
fi
echo "sync failed" >&2
exit 12
"#,
        )
        .expect("write script");
        let mut perms = fs::metadata(&script_path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).expect("chmod");

        let runtime = DotagentsRuntimeManager::new(temp.path().to_path_buf())
            .with_override_binary(script_path);
        let adapter = DotagentsAdapter::new(runtime);
        let error = adapter
            .run(&["sync"], temp.path(), false)
            .expect_err("command should fail");
        let message = error.to_string();

        assert!(message.contains("dotagents command failed"));
        assert!(message.contains("sync failed"));
    }
}
