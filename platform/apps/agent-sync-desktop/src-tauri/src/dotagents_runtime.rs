use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;

const EXPECTED_DOTAGENTS_VERSION: &str = "1.4.0";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DotagentsRuntimeStatus {
    pub available: bool,
    pub expected_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DotagentsRuntimeManager {
    expected_version: String,
    npx_binary: Option<PathBuf>,
}

impl Default for DotagentsRuntimeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DotagentsRuntimeManager {
    pub fn new() -> Self {
        Self {
            expected_version: EXPECTED_DOTAGENTS_VERSION.to_string(),
            npx_binary: None,
        }
    }

    pub fn expected_version(&self) -> &str {
        &self.expected_version
    }

    pub fn npx_package_spec(&self) -> String {
        format!("@sentry/dotagents@{}", self.expected_version)
    }

    pub fn check_npx_available(&self) -> Result<(), String> {
        self.npx_command()
            .arg("--version")
            .output()
            .map_err(|_| String::from("npx is not available — install Node.js and npm"))?;
        Ok(())
    }

    pub fn check_pinned_cli_available(&self) -> Result<(), String> {
        self.check_npx_available()?;

        let output = self
            .npx_command()
            .arg("--yes")
            .arg(self.npx_package_spec())
            .arg("--version")
            .output()
            .map_err(|error| format!("failed to launch npx: {error}"))?;

        if !output.status.success() {
            return Err(format!(
                "failed to resolve {} via npx: {}",
                self.npx_package_spec(),
                combine_output(&output.stderr, &output.stdout)
            ));
        }

        let version_output = combine_output(&output.stdout, &output.stderr);
        if output_contains_exact_version(&version_output, self.expected_version()) {
            return Ok(());
        }

        Err(format!(
            "dotagents version mismatch: expected {}, got {}",
            self.expected_version(),
            version_output
        ))
    }

    fn npx_command(&self) -> Command {
        match &self.npx_binary {
            Some(path) => Command::new(path),
            None => Command::new("npx"),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_npx_binary(mut self, path: PathBuf) -> Self {
        self.npx_binary = Some(path);
        self
    }
}

fn combine_output(primary: &[u8], secondary: &[u8]) -> String {
    let primary = String::from_utf8_lossy(primary).trim().to_string();
    let secondary = String::from_utf8_lossy(secondary).trim().to_string();

    match (primary.is_empty(), secondary.is_empty()) {
        (false, true) => primary,
        (true, false) => secondary,
        (false, false) => format!("{primary}\n{secondary}"),
        (true, true) => String::from("unknown error"),
    }
}

fn output_contains_exact_version(raw_output: &str, expected_version: &str) -> bool {
    raw_output
        .split_whitespace()
        .any(|token| token.trim() == expected_version)
}

#[cfg(test)]
mod tests {
    use super::DotagentsRuntimeManager;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    #[test]
    fn returns_expected_version() {
        let manager = DotagentsRuntimeManager::new();
        assert_eq!(manager.expected_version(), "1.4.0");
    }

    #[test]
    fn npx_package_spec_includes_version() {
        let manager = DotagentsRuntimeManager::new();
        assert_eq!(manager.npx_package_spec(), "@sentry/dotagents@1.4.0");
    }

    #[test]
    #[cfg(unix)]
    fn pinned_cli_probe_accepts_expected_version() {
        let temp = tempdir().expect("tempdir");
        let script_path = temp.path().join("npx");
        fs::write(
            &script_path,
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "10.0.0"
  exit 0
fi
if [ "$1" = "--yes" ] && [ "$2" = "@sentry/dotagents@1.4.0" ] && [ "$3" = "--version" ]; then
  echo "1.4.0"
  exit 0
fi
echo "unexpected invocation" >&2
exit 1
"#,
        )
        .expect("write script");
        let mut perms = fs::metadata(&script_path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).expect("chmod");

        let manager = DotagentsRuntimeManager::new().with_npx_binary(script_path);
        manager
            .check_pinned_cli_available()
            .expect("probe should succeed");
    }

    #[test]
    #[cfg(unix)]
    fn pinned_cli_probe_rejects_unresolvable_package() {
        let temp = tempdir().expect("tempdir");
        let script_path = temp.path().join("npx");
        fs::write(
            &script_path,
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "10.0.0"
  exit 0
fi
echo "npm ERR! network timeout" >&2
exit 1
"#,
        )
        .expect("write script");
        let mut perms = fs::metadata(&script_path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).expect("chmod");

        let manager = DotagentsRuntimeManager::new().with_npx_binary(script_path);
        let error = manager
            .check_pinned_cli_available()
            .expect_err("probe should fail");
        assert!(error.contains("failed to resolve @sentry/dotagents@1.4.0 via npx"));
    }
}
