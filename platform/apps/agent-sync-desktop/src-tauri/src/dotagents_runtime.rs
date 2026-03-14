use serde::Serialize;
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const EXPECTED_DOTAGENTS_VERSION: &str = "0.10.0";
const CHECKSUM_MANIFEST_FILE: &str = "checksums.json";
const DOTAGENTS_TARGET_PREFIX: &str = "dotagents";
const OVERRIDE_BIN_ENV: &str = "DOTAGENTS_DESKTOP_DOTAGENTS_BIN";
const OVERRIDE_BUNDLE_DIR_ENV: &str = "DOTAGENTS_DESKTOP_DOTAGENTS_BUNDLE_DIR";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DotagentsBinarySource {
    Override,
    Bundled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DotagentsTarget {
    os: &'static str,
    arch: &'static str,
}

impl DotagentsTarget {
    fn current() -> Result<Self, String> {
        let os = match env::consts::OS {
            "macos" => "darwin",
            "linux" => "linux",
            "windows" => "windows",
            other => {
                return Err(format!("unsupported operating system: {other}"));
            }
        };

        let arch = match env::consts::ARCH {
            "x86_64" => "x64",
            "aarch64" => "arm64",
            other => {
                return Err(format!("unsupported architecture: {other}"));
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DotagentsRuntimeStatus {
    pub available: bool,
    pub expected_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DotagentsRuntimeManager {
    expected_version: String,
    override_binary: Option<PathBuf>,
    bundled_root_override: Option<PathBuf>,
    checksum_override: Option<String>,
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
            override_binary: None,
            bundled_root_override: None,
            checksum_override: None,
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

    pub fn expected_version(&self) -> &str {
        &self.expected_version
    }

    pub fn resolve_binary(&self) -> Result<DotagentsResolvedBinary, String> {
        let target = DotagentsTarget::current()?;

        if let Some(path) = self.resolve_override_path() {
            self.ensure_binary_exists(&path)?;
            return Ok(DotagentsResolvedBinary {
                path,
                source: DotagentsBinarySource::Override,
                target,
            });
        }

        for path in self.find_bundled_binary_candidates(target) {
            self.ensure_binary_exists(&path)?;
            if self.has_checksum_manifest_for_binary(&path, target) {
                return Ok(DotagentsResolvedBinary {
                    path,
                    source: DotagentsBinarySource::Bundled,
                    target,
                });
            }
        }

        Err(String::from(
            "bundled dotagents binary not found; packaged runtime is required",
        ))
    }

    pub fn verify_checksum(&self, binary: &DotagentsResolvedBinary) -> Result<(), String> {
        if binary.source != DotagentsBinarySource::Bundled {
            return Ok(());
        }

        let expected = self
            .checksum_override
            .clone()
            .or_else(|| self.expected_checksum_for_bundled_binary_path(&binary.path, binary.target))
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                format!(
                    "checksum manifest missing for bundled target {}",
                    binary.target.identifier()
                )
            })?;

        let actual = sha256_file(&binary.path)?;
        if actual.eq_ignore_ascii_case(expected.trim()) {
            return Ok(());
        }

        Err(format!(
            "checksum mismatch for {}: expected {}, got {}",
            binary.path.display(),
            expected,
            actual
        ))
    }

    pub fn parse_version_output(&self, raw_output: &str) -> Result<String, String> {
        let compact = raw_output.trim();
        if output_contains_exact_version(compact, self.expected_version()) {
            return Ok(self.expected_version().to_string());
        }

        Err(format!(
            "dotagents version mismatch: expected {}, got {}",
            self.expected_version(),
            compact
        ))
    }

    fn resolve_override_path(&self) -> Option<PathBuf> {
        if let Some(path) = &self.override_binary {
            return Some(path.clone());
        }

        env::var(OVERRIDE_BIN_ENV)
            .ok()
            .map(|raw| raw.trim().to_owned())
            .filter(|raw| !raw.is_empty())
            .map(PathBuf::from)
    }

    fn find_bundled_binary_candidates(&self, target: DotagentsTarget) -> Vec<PathBuf> {
        let mut candidates = Vec::new();
        for root in self.bundled_candidate_roots() {
            for prefix in [
                PathBuf::from(DOTAGENTS_TARGET_PREFIX),
                PathBuf::from("bin").join(DOTAGENTS_TARGET_PREFIX),
            ] {
                let target_dir = root.join(&prefix).join(target.identifier());
                for binary_name in dotagents_binary_candidates() {
                    let candidate = target_dir.join(binary_name);
                    if candidate.exists() {
                        candidates.push(candidate);
                    }
                }
            }
        }

        dedup_paths(candidates)
    }

    fn bundled_candidate_roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();

        if let Some(root) = &self.bundled_root_override {
            roots.push(root.clone());
        }

        if let Ok(raw_root) = env::var(OVERRIDE_BUNDLE_DIR_ENV) {
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

    fn has_checksum_manifest_for_binary(
        &self,
        binary_path: &Path,
        target: DotagentsTarget,
    ) -> bool {
        self.expected_checksum_for_bundled_binary_path(binary_path, target)
            .as_deref()
            .map(str::trim)
            .map(|value| !value.is_empty())
            .unwrap_or(false)
    }

    fn expected_checksum_for_bundled_binary_path(
        &self,
        binary_path: &Path,
        target: DotagentsTarget,
    ) -> Option<String> {
        if let Some(checksum) = self
            .checksum_override
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(checksum.to_string());
        }

        let manifest_path = bundled_manifest_path(binary_path)?;
        checksum_from_manifest(&manifest_path, target)
    }

    fn ensure_binary_exists(&self, path: &Path) -> Result<(), String> {
        if path.exists() {
            return Ok(());
        }

        Err(format!(
            "dotagents binary does not exist: {}",
            path.display()
        ))
    }
}

fn dotagents_binary_candidates() -> &'static [&'static str] {
    if cfg!(windows) {
        &["dotagents.cmd", "dotagents.exe", "dotagents"]
    } else {
        &["dotagents"]
    }
}

fn bundled_manifest_path(binary_path: &Path) -> Option<PathBuf> {
    let target_dir = binary_path.parent()?;
    let bundle_dir = target_dir.parent()?;
    Some(bundle_dir.join(CHECKSUM_MANIFEST_FILE))
}

fn checksum_from_manifest(path: &Path, target: DotagentsTarget) -> Option<String> {
    let contents = fs::read_to_string(path).ok()?;
    let parsed = serde_json::from_str::<serde_json::Value>(&contents).ok()?;
    parsed
        .get("checksums")?
        .get(target.identifier())?
        .as_str()
        .map(str::to_string)
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("failed to read {} for checksum: {error}", path.display()))?;
    let digest = Sha256::digest(bytes);
    Ok(format!("{digest:x}"))
}

fn output_contains_exact_version(raw_output: &str, expected_version: &str) -> bool {
    raw_output
        .split_whitespace()
        .any(|token| token.trim() == expected_version)
}

fn dedup_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut deduped = Vec::new();
    for path in paths {
        if deduped.iter().any(|existing| existing == &path) {
            continue;
        }
        deduped.push(path);
    }
    deduped
}

#[cfg(test)]
mod tests {
    use super::{DotagentsBinarySource, DotagentsRuntimeManager};
    use sha2::Digest;
    use std::fs;
    use std::sync::{Mutex, OnceLock};
    use tempfile::tempdir;

    fn env_lock() -> &'static Mutex<()> {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        ENV_LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn resolve_binary_rejects_path_fallback() {
        let _guard = env_lock().lock().expect("env lock");
        let original_path = std::env::var_os("PATH");

        let temp = tempdir().expect("tempdir");
        let bin_dir = temp.path().join("bin");
        fs::create_dir_all(&bin_dir).expect("bin dir");
        fs::write(bin_dir.join("dotagents"), b"fake").expect("fake path binary");
        std::env::set_var("PATH", &bin_dir);

        let manager = DotagentsRuntimeManager::new();
        let error = manager
            .resolve_binary()
            .expect_err("path fallback must be rejected");

        match original_path {
            Some(value) => std::env::set_var("PATH", value),
            None => std::env::remove_var("PATH"),
        }

        assert!(error.contains("bundled dotagents binary not found"));
    }

    #[test]
    fn resolve_binary_uses_override_path() {
        let temp = tempdir().expect("tempdir");
        let binary_path = temp.path().join("dotagents");
        fs::write(&binary_path, b"fake").expect("write fake binary");

        let manager = DotagentsRuntimeManager::new().with_override_binary(binary_path.clone());
        let resolved = manager.resolve_binary().expect("resolve override");

        assert_eq!(resolved.path, binary_path);
        assert_eq!(resolved.source, DotagentsBinarySource::Override);
    }

    #[test]
    fn resolve_binary_accepts_bundled_layout() {
        let temp = tempdir().expect("tempdir");
        let target = super::DotagentsTarget::current().expect("target");
        let target_dir = temp
            .path()
            .join("bin")
            .join("dotagents")
            .join(target.identifier());
        fs::create_dir_all(&target_dir).expect("target dir");
        let binary_name = if cfg!(windows) {
            "dotagents.cmd"
        } else {
            "dotagents"
        };
        let binary_path = target_dir.join(binary_name);
        fs::write(&binary_path, b"fake bundled binary").expect("write bundled binary");
        let checksum = format!("{:x}", sha2::Sha256::digest(b"fake bundled binary"));
        fs::write(
            temp.path()
                .join("bin")
                .join("dotagents")
                .join("checksums.json"),
            format!(
                "{{\"checksums\":{{\"{}\":\"{}\"}}}}",
                target_dir.file_name().unwrap().to_string_lossy(),
                checksum
            ),
        )
        .expect("write manifest");

        let manager =
            DotagentsRuntimeManager::new().with_bundled_root_override(temp.path().to_path_buf());
        let resolved = manager.resolve_binary().expect("resolve bundled");

        assert_eq!(resolved.path, binary_path);
        manager.verify_checksum(&resolved).expect("checksum");
    }
}
