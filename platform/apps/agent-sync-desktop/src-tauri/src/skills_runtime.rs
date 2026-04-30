use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;

// The Skills CLI moves quickly and we have no signal that any specific
// version is "blessed" by the vendor. We default to "latest" and let a
// user override it via settings if a specific pin becomes desirable.
// TODO: revisit once `skills` publishes a 1.x stable; pin then.
const DEFAULT_SKILLS_VERSION: &str = "latest";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillsRuntimeStatus {
    pub available: bool,
    pub expected_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SkillsRuntimeManager {
    expected_version: String,
    npx_binary: Option<PathBuf>,
}

impl Default for SkillsRuntimeManager {
    fn default() -> Self {
        Self::new(None)
    }
}

impl SkillsRuntimeManager {
    pub fn new(version_override: Option<String>) -> Self {
        let raw = version_override
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or(DEFAULT_SKILLS_VERSION)
            .to_string();
        Self {
            expected_version: raw,
            npx_binary: None,
        }
    }

    pub fn expected_version(&self) -> &str {
        &self.expected_version
    }

    pub fn npx_package_spec(&self) -> String {
        format!("skills@{}", self.expected_version)
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

        Ok(())
    }

    pub(crate) fn npx_command(&self) -> Command {
        match &self.npx_binary {
            Some(path) => Command::new(path),
            None => Command::new("npx"),
        }
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn with_npx_binary(mut self, path: PathBuf) -> Self {
        self.npx_binary = Some(path);
        self
    }
}

/// Validate a user-supplied version override (allows `latest`, semver,
/// semver ranges; rejects anything that looks like a path).
pub fn validate_version_override(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(String::from("version override cannot be empty"));
    }
    if trimmed.contains('/') || trimmed.contains('\\') || trimmed.starts_with("file:") {
        return Err(String::from(
            "version override must be 'latest', a semver, or a semver range — not a path",
        ));
    }
    // Permit:
    //   latest
    //   ^1.2.3, ~1.2.3, >=1.0.0 <2.0.0, 1.x, 1
    // Reject any whitespace inside? — npm allows ranges with spaces (">=1 <2"),
    // so we allow spaces but require at least one non-space character.
    Ok(trimmed.to_string())
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

#[cfg(test)]
mod tests {
    use super::{validate_version_override, SkillsRuntimeManager};

    #[test]
    fn defaults_to_latest_when_no_override() {
        let manager = SkillsRuntimeManager::new(None);
        assert_eq!(manager.expected_version(), "latest");
        assert_eq!(manager.npx_package_spec(), "skills@latest");
    }

    #[test]
    fn override_takes_effect_in_package_spec() {
        let manager = SkillsRuntimeManager::new(Some(String::from("0.4.0")));
        assert_eq!(manager.npx_package_spec(), "skills@0.4.0");
    }

    #[test]
    fn empty_override_falls_back_to_default() {
        let manager = SkillsRuntimeManager::new(Some(String::from("   ")));
        assert_eq!(manager.expected_version(), "latest");
    }

    #[test]
    fn validate_accepts_semver_and_range() {
        assert!(validate_version_override("latest").is_ok());
        assert!(validate_version_override("1.2.3").is_ok());
        assert!(validate_version_override("^1.2.3").is_ok());
        assert!(validate_version_override(">=1.0.0 <2.0.0").is_ok());
    }

    #[test]
    fn validate_rejects_paths() {
        assert!(validate_version_override("/abs/path").is_err());
        assert!(validate_version_override("file:./local").is_err());
        assert!(validate_version_override("../local").is_err());
        assert!(validate_version_override("").is_err());
    }
}
