use directories::ProjectDirs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SyncPaths {
    pub runtime_directory: PathBuf,
    pub state_path: PathBuf,
    pub app_settings_path: PathBuf,
}

impl SyncPaths {
    pub fn detect() -> Self {
        if let Ok(override_dir) = std::env::var("SKILLS_SYNC_GROUP_DIR") {
            if !override_dir.trim().is_empty() {
                let runtime = PathBuf::from(override_dir);
                return Self::from_runtime(runtime);
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Some(home) = home_dir() {
                let runtime = home
                    .join("Library")
                    .join("Application Support")
                    .join("SkillsSync");
                return Self::from_runtime(runtime);
            }
        }

        if let Some(project_dirs) = ProjectDirs::from("dev", "fedosov", "SkillsSync") {
            return Self::from_runtime(project_dirs.data_dir().to_path_buf());
        }

        if let Some(home) = home_dir() {
            return Self::from_runtime(home.join(".skillssync"));
        }

        Self::from_runtime(PathBuf::from(".skillssync"))
    }

    pub fn from_runtime(runtime_directory: PathBuf) -> Self {
        let state_path = runtime_directory.join("state.json");
        let app_settings_path = runtime_directory.join("app-settings.json");
        Self {
            runtime_directory,
            state_path,
            app_settings_path,
        }
    }

    pub fn ensure_runtime_dir(&self) -> Result<(), std::io::Error> {
        std::fs::create_dir_all(&self.runtime_directory)
    }
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| directories::BaseDirs::new().map(|d| d.home_dir().to_path_buf()))
}

pub fn standardized(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
