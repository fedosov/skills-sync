use std::path::{Path, PathBuf};
use std::process::Command;

pub fn open_path(path: &Path) -> Result<(), String> {
    let target = existing_target(path);

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(&target);
        command
    };

    #[cfg(target_os = "linux")]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(&target);
        command
    };

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("cmd");
        command.arg("/C").arg("start").arg("").arg(&target);
        command
    };

    let status = command
        .status()
        .map_err(|error| format!("failed to launch opener for {}: {error}", target.display()))?;
    if status.success() {
        return Ok(());
    }

    Err(format!(
        "opener exited with status {status} for {}",
        target.display()
    ))
}

fn existing_target(path: &Path) -> PathBuf {
    if path.exists() {
        return path.to_path_buf();
    }

    path.parent()
        .filter(|parent| parent.exists())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| path.to_path_buf())
}
