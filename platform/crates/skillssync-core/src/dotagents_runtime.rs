use crate::error::SyncEngineError;
use sha2::{Digest, Sha256};
use std::env;
use std::path::{Path, PathBuf};

const DEFAULT_DOTAGENTS_VERSION: &str = "0.10.0";
const DOTAGENTS_BINARY_NAME: &str = "dotagents";
const DOTAGENTS_TARGET_PREFIX: &str = "dotagents";

const TARGET_CHECKSUMS: &[(&str, &str, &str)] = &[
    ("darwin", "arm64", ""),
    ("darwin", "x64", ""),
    ("linux", "x64", ""),
    ("linux", "arm64", ""),
    ("windows", "x64", ""),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DotagentsBinarySource {
    Override,
    Bundled,
    SystemPath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DotagentsTarget {
    os: &'static str,
    arch: &'static str,
}

impl DotagentsTarget {
    fn current() -> Result<Self, SyncEngineError> {
        let os = match env::consts::OS {
            "macos" => "darwin",
            "linux" => "linux",
            "windows" => "windows",
            other => {
                return Err(SyncEngineError::DotagentsUnavailable(format!(
                    "unsupported operating system: {other}"
                )));
            }
        };

        let arch = match env::consts::ARCH {
            "x86_64" => "x64",
            "aarch64" => "arm64",
            other => {
                return Err(SyncEngineError::DotagentsUnavailable(format!(
                    "unsupported architecture: {other}"
                )));
            }
        };

        Ok(Self { os, arch })
    }

    fn identifier(self) -> String {
        format!("{}-{}", self.os, self.arch)
    }
}

#[derive(Debug, Clone)]
pub struct DotagentsResolvedBinary {
    pub path: PathBuf,
    pub source: DotagentsBinarySource,
    pub target: DotagentsTarget,
}

#[derive(Debug, Clone)]
pub struct DotagentsRuntimeManager {
    home_directory: PathBuf,
    expected_version: String,
    override_binary: Option<PathBuf>,
    bundled_root_override: Option<PathBuf>,
    checksum_override: Option<String>,
    #[cfg(test)]
    disable_system_path_lookup: bool,
}

impl DotagentsRuntimeManager {
    pub fn new(home_directory: PathBuf) -> Self {
        Self {
            home_directory,
            expected_version: DEFAULT_DOTAGENTS_VERSION.to_string(),
            override_binary: None,
            bundled_root_override: None,
            checksum_override: None,
            #[cfg(test)]
            disable_system_path_lookup: false,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_override_binary(mut self, path: PathBuf) -> Self {
        self.override_binary = Some(path);
        self
    }

    #[cfg(test)]
    pub(crate) fn with_bundled_root_override(mut self, root: PathBuf) -> Self {
        self.bundled_root_override = Some(root);
        self
    }

    #[cfg(test)]
    pub(crate) fn with_checksum_override(mut self, checksum: impl Into<String>) -> Self {
        self.checksum_override = Some(checksum.into());
        self
    }

    #[cfg(test)]
    pub(crate) fn with_system_path_lookup_disabled(mut self) -> Self {
        self.disable_system_path_lookup = true;
        self
    }

    pub fn home_directory(&self) -> &Path {
        &self.home_directory
    }

    pub fn expected_version(&self) -> &str {
        &self.expected_version
    }

    pub fn resolve_binary(&self) -> Result<DotagentsResolvedBinary, SyncEngineError> {
        let target = DotagentsTarget::current()?;

        if let Some(path) = self.resolve_override_path() {
            self.ensure_binary_exists(&path)?;
            return Ok(DotagentsResolvedBinary {
                path,
                source: DotagentsBinarySource::Override,
                target,
            });
        }

        if let Some(path) = self.find_bundled_binary(target) {
            self.ensure_binary_exists(&path)?;
            if self.has_checksum_manifest_for_target(target) {
                return Ok(DotagentsResolvedBinary {
                    path,
                    source: DotagentsBinarySource::Bundled,
                    target,
                });
            }
        }

        if let Some(path) = self.find_in_path() {
            self.ensure_binary_exists(&path)?;
            return Ok(DotagentsResolvedBinary {
                path,
                source: DotagentsBinarySource::SystemPath,
                target,
            });
        }

        Err(SyncEngineError::DotagentsUnavailable(String::from(
            "dotagents binary not found (override, bundled resources, or PATH)",
        )))
    }

    pub fn verify_checksum(&self, binary: &DotagentsResolvedBinary) -> Result<(), SyncEngineError> {
        if binary.source != DotagentsBinarySource::Bundled {
            return Ok(());
        }

        let expected = self
            .checksum_override
            .clone()
            .or_else(|| expected_checksum_for_target(binary.target))
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                SyncEngineError::DotagentsUnavailable(format!(
                    "checksum manifest missing for bundled target {}",
                    binary.target.identifier()
                ))
            })?;

        let actual = sha256_file(&binary.path)?;
        if !actual.eq_ignore_ascii_case(expected.trim()) {
            return Err(SyncEngineError::DotagentsChecksumMismatch {
                path: binary.path.clone(),
                expected,
                actual,
            });
        }

        Ok(())
    }

    pub fn verify_version_output(&self, raw_output: &str) -> Result<(), SyncEngineError> {
        let compact = raw_output.trim();
        if output_contains_exact_version(compact, self.expected_version()) {
            return Ok(());
        }

        Err(SyncEngineError::DotagentsVersionMismatch {
            expected: self.expected_version().to_string(),
            actual: compact.to_string(),
        })
    }

    fn resolve_override_path(&self) -> Option<PathBuf> {
        if let Some(path) = &self.override_binary {
            return Some(path.clone());
        }

        env::var("SKILLS_SYNC_DOTAGENTS_BIN")
            .ok()
            .map(|raw| raw.trim().to_owned())
            .filter(|raw| !raw.is_empty())
            .map(PathBuf::from)
    }

    fn find_bundled_binary(&self, target: DotagentsTarget) -> Option<PathBuf> {
        let binary_name = dotagents_binary_name();

        for root in self.bundled_candidate_roots() {
            for prefix in [
                PathBuf::from(DOTAGENTS_TARGET_PREFIX),
                PathBuf::from("bin").join(DOTAGENTS_TARGET_PREFIX),
            ] {
                let candidate = root
                    .join(&prefix)
                    .join(target.identifier())
                    .join(binary_name);
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }

        None
    }

    fn has_checksum_manifest_for_target(&self, target: DotagentsTarget) -> bool {
        if self
            .checksum_override
            .as_deref()
            .map(str::trim)
            .map(|value| !value.is_empty())
            .unwrap_or(false)
        {
            return true;
        }

        expected_checksum_for_target(target)
            .as_deref()
            .map(str::trim)
            .map(|value| !value.is_empty())
            .unwrap_or(false)
    }

    fn bundled_candidate_roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();

        if let Some(root) = &self.bundled_root_override {
            roots.push(root.clone());
        }

        if let Ok(raw_root) = env::var("SKILLS_SYNC_DOTAGENTS_BUNDLE_DIR") {
            let trimmed = raw_root.trim();
            if !trimmed.is_empty() {
                roots.push(PathBuf::from(trimmed));
            }
        }

        if let Ok(exe_path) = env::current_exe() {
            if let Some(parent) = exe_path.parent() {
                roots.push(parent.to_path_buf());
                roots.push(parent.join("resources"));
                roots.push(parent.join("Resources"));

                if let Some(grand_parent) = parent.parent() {
                    roots.push(grand_parent.join("resources"));
                    roots.push(grand_parent.join("Resources"));
                }
            }
        }

        dedup_paths(roots)
    }

    fn find_in_path(&self) -> Option<PathBuf> {
        #[cfg(test)]
        if self.disable_system_path_lookup {
            return None;
        }

        let path_env = env::var_os("PATH")?;
        let binary_name = dotagents_binary_name();
        env::split_paths(&path_env)
            .map(|entry| entry.join(binary_name))
            .find(|candidate| candidate.exists())
    }

    fn ensure_binary_exists(&self, path: &Path) -> Result<(), SyncEngineError> {
        if path.exists() {
            return Ok(());
        }

        Err(SyncEngineError::DotagentsUnavailable(format!(
            "dotagents binary does not exist: {}",
            path.display()
        )))
    }
}

fn output_contains_exact_version(raw_output: &str, expected: &str) -> bool {
    raw_output
        .split_whitespace()
        .map(|token| token.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '.'))
        .map(|token| token.strip_prefix('v').unwrap_or(token))
        .any(|token| is_numeric_version_token(token) && token == expected)
}

fn is_numeric_version_token(token: &str) -> bool {
    !token.is_empty()
        && token
            .split('.')
            .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
}

fn expected_checksum_for_target(target: DotagentsTarget) -> Option<String> {
    TARGET_CHECKSUMS
        .iter()
        .find(|(os, arch, _)| *os == target.os && *arch == target.arch)
        .map(|(_, _, checksum)| checksum.to_string())
}

fn dotagents_binary_name() -> &'static str {
    if env::consts::OS == "windows" {
        "dotagents.exe"
    } else {
        DOTAGENTS_BINARY_NAME
    }
}

fn sha256_file(path: &Path) -> Result<String, SyncEngineError> {
    let bytes = std::fs::read(path).map_err(|error| SyncEngineError::io(path, error))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn dedup_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut unique = std::collections::HashSet::new();
    let mut result = Vec::new();

    for path in paths {
        let key = path.to_string_lossy().to_string();
        if unique.insert(key) {
            result.push(path);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::{dotagents_binary_name, DotagentsBinarySource, DotagentsRuntimeManager};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn resolve_binary_prefers_override_path() {
        let temp = TempDir::new().expect("tempdir");
        let binary_path = temp.path().join(dotagents_binary_name());
        fs::write(&binary_path, "#!/bin/sh\necho ok\n").expect("write fake binary");

        let manager = DotagentsRuntimeManager::new(temp.path().to_path_buf())
            .with_override_binary(binary_path.clone());
        let resolved = manager.resolve_binary().expect("resolve binary");

        assert_eq!(resolved.path, binary_path);
        assert_eq!(resolved.source, DotagentsBinarySource::Override);
    }

    #[test]
    fn verify_version_output_rejects_mismatch() {
        let temp = TempDir::new().expect("tempdir");
        let manager = DotagentsRuntimeManager::new(temp.path().to_path_buf());

        manager
            .verify_version_output("dotagents 0.10.0")
            .expect("version should match");

        let error = manager
            .verify_version_output("dotagents 9.9.9")
            .expect_err("version mismatch expected");
        assert!(error.to_string().contains("version mismatch"));
    }

    #[test]
    fn verify_version_output_rejects_superset_version_strings() {
        let temp = TempDir::new().expect("tempdir");
        let manager = DotagentsRuntimeManager::new(temp.path().to_path_buf());

        let error = manager
            .verify_version_output("dotagents 10.10.0")
            .expect_err("superset version must be rejected");
        assert!(error.to_string().contains("version mismatch"));
    }

    #[test]
    fn verify_checksum_reports_mismatch_for_bundled_binary() {
        let temp = TempDir::new().expect("tempdir");
        let binary_dir =
            temp.path()
                .join("dotagents")
                .join(format!("{}-{}", runtime_os(), runtime_arch()));
        fs::create_dir_all(&binary_dir).expect("create binary dir");
        let binary_path = binary_dir.join(dotagents_binary_name());
        fs::write(&binary_path, "binary").expect("write fake binary");

        let manager = DotagentsRuntimeManager::new(temp.path().to_path_buf())
            .with_bundled_root_override(temp.path().to_path_buf())
            .with_checksum_override("deadbeef");

        let resolved = manager.resolve_binary().expect("resolve bundled binary");
        assert_eq!(resolved.source, DotagentsBinarySource::Bundled);

        let error = manager
            .verify_checksum(&resolved)
            .expect_err("checksum mismatch expected");
        assert!(error.to_string().contains("checksum mismatch"));
    }

    #[test]
    fn resolve_binary_skips_unverifiable_bundled_binary() {
        let temp = TempDir::new().expect("tempdir");
        let binary_dir =
            temp.path()
                .join("dotagents")
                .join(format!("{}-{}", runtime_os(), runtime_arch()));
        fs::create_dir_all(&binary_dir).expect("create binary dir");
        let binary_path = binary_dir.join(dotagents_binary_name());
        fs::write(&binary_path, "binary").expect("write fake binary");

        let manager = DotagentsRuntimeManager::new(temp.path().to_path_buf())
            .with_bundled_root_override(temp.path().to_path_buf())
            .with_system_path_lookup_disabled();

        let error = manager
            .resolve_binary()
            .expect_err("bundled binary without checksum must be skipped");
        assert!(error.to_string().contains("dotagents binary not found"));
    }

    #[test]
    fn resolve_binary_finds_bundled_binary_in_bin_dotagents_layout() {
        let temp = TempDir::new().expect("tempdir");
        let binary_dir = temp.path().join("bin").join("dotagents").join(format!(
            "{}-{}",
            runtime_os(),
            runtime_arch()
        ));
        fs::create_dir_all(&binary_dir).expect("create binary dir");
        let binary_path = binary_dir.join(dotagents_binary_name());
        fs::write(&binary_path, "binary").expect("write fake binary");

        let manager = DotagentsRuntimeManager::new(temp.path().to_path_buf())
            .with_bundled_root_override(temp.path().to_path_buf())
            .with_checksum_override("test-checksum");
        let resolved = manager.resolve_binary().expect("resolve bundled binary");

        assert_eq!(resolved.source, DotagentsBinarySource::Bundled);
        assert_eq!(resolved.path, binary_path);
    }

    fn runtime_os() -> &'static str {
        match std::env::consts::OS {
            "macos" => "darwin",
            "linux" => "linux",
            "windows" => "windows",
            _ => "unknown",
        }
    }

    fn runtime_arch() -> &'static str {
        match std::env::consts::ARCH {
            "x86_64" => "x64",
            "aarch64" => "arm64",
            _ => "unknown",
        }
    }
}
