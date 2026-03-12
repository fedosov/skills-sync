use agent_sync_core::{SkillRecord, SubagentRecord};
use serde::Serialize;
use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;

pub(crate) const MAX_MAIN_FILE_PREVIEW_CHARS: usize = 50_000;
pub(crate) const MAX_TREE_ENTRIES: usize = 500;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct PlatformContext {
    pub(crate) os: String,
    pub(crate) linux_desktop: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SkillDetails {
    pub(crate) skill: SkillRecord,
    pub(crate) main_file_path: String,
    pub(crate) main_file_exists: bool,
    pub(crate) main_file_body_preview: Option<String>,
    pub(crate) skill_dir_tree_preview: Option<String>,
    pub(crate) last_modified_unix_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SubagentDetails {
    pub(crate) subagent: SubagentRecord,
    pub(crate) main_file_path: String,
    pub(crate) main_file_exists: bool,
    pub(crate) main_file_body_preview: Option<String>,
    pub(crate) last_modified_unix_seconds: Option<u64>,
}

pub(crate) fn last_modified_seconds(path: &Path) -> Option<u64> {
    fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|ts| ts.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
}

pub(crate) fn normalize_os_name(raw_os: &str) -> &'static str {
    match raw_os {
        "macos" => "macos",
        "windows" => "windows",
        "linux" => "linux",
        _ => "unknown",
    }
}

pub(crate) fn build_platform_context(
    raw_os: &str,
    linux_desktop_raw: Option<&str>,
) -> PlatformContext {
    let os = normalize_os_name(raw_os);
    let linux_desktop = if os == "linux" {
        normalize_linux_desktop(linux_desktop_raw)
    } else {
        None
    };
    PlatformContext {
        os: os.to_owned(),
        linux_desktop,
    }
}

pub(crate) fn resolve_main_skill_file(skill: &SkillRecord) -> PathBuf {
    let source = PathBuf::from(&skill.canonical_source_path);
    if skill.package_type == "dir" {
        source.join("SKILL.md")
    } else {
        source
    }
}

pub(crate) fn resolve_skill_root_dir(skill: &SkillRecord, main_file: &Path) -> PathBuf {
    if skill.package_type == "dir" {
        return PathBuf::from(&skill.canonical_source_path);
    }
    main_file
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(&skill.canonical_source_path))
}

pub(crate) fn read_preview(path: &Path, max_chars: usize) -> (Option<String>, bool) {
    let Ok(contents) = fs::read_to_string(path) else {
        return (None, false);
    };

    let total_chars = contents.chars().count();
    if total_chars <= max_chars {
        return (Some(contents), false);
    }

    let preview = contents.chars().take(max_chars).collect::<String>();
    (Some(preview), true)
}

pub(crate) fn read_skill_dir_tree(root: &Path, max_entries: usize) -> (Option<String>, bool) {
    if max_entries == 0 {
        return (None, false);
    }

    let Ok(metadata) = fs::symlink_metadata(root) else {
        return (None, false);
    };
    if !metadata.file_type().is_dir() {
        return (None, false);
    }

    let mut lines = Vec::new();
    let root_label = root
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| root.display().to_string());
    lines.push(format!("{root_label}/"));

    let mut emitted_entries: usize = 0;
    let mut truncated = false;
    render_tree_entries(
        root,
        "",
        &mut lines,
        max_entries,
        &mut emitted_entries,
        &mut truncated,
    );

    (Some(lines.join("\n")), truncated)
}

pub(crate) fn open_path(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("path does not exist: {}", path.display()));
    }

    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut cmd = Command::new("open");
        cmd.arg(path);
        cmd
    };

    #[cfg(target_os = "linux")]
    let mut cmd = {
        let mut cmd = Command::new("xdg-open");
        cmd.arg(path);
        cmd
    };

    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg("start").arg("").arg(path);
        cmd
    };

    let status = cmd
        .status()
        .map_err(|error| format!("failed to launch opener for {}: {}", path.display(), error))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "opener exited with status {} for {}",
            status,
            path.display()
        ))
    }
}

fn normalize_linux_desktop(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn render_tree_entries(
    dir: &Path,
    prefix: &str,
    lines: &mut Vec<String>,
    max_entries: usize,
    emitted_entries: &mut usize,
    truncated: &mut bool,
) {
    if *truncated {
        return;
    }

    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    let mut children: Vec<(String, bool, PathBuf)> = entries
        .filter_map(Result::ok)
        .map(|entry| {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = fs::symlink_metadata(&path)
                .map(|meta| meta.file_type().is_dir())
                .unwrap_or(false);
            (name, is_dir, path)
        })
        .collect();

    children.sort_by(|lhs, rhs| match (lhs.1, rhs.1) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => lhs
            .0
            .to_lowercase()
            .cmp(&rhs.0.to_lowercase())
            .then_with(|| lhs.0.cmp(&rhs.0)),
    });

    let child_count = children.len();
    for (index, (name, is_dir, path)) in children.into_iter().enumerate() {
        if *emitted_entries >= max_entries {
            *truncated = true;
            return;
        }

        let is_last = index + 1 == child_count;
        let branch = if is_last { "`-- " } else { "|-- " };
        let label = if is_dir { format!("{name}/") } else { name };
        lines.push(format!("{prefix}{branch}{label}"));
        *emitted_entries += 1;

        if is_dir {
            let next_prefix = if is_last {
                format!("{prefix}    ")
            } else {
                format!("{prefix}|   ")
            };
            render_tree_entries(
                &path,
                &next_prefix,
                lines,
                max_entries,
                emitted_entries,
                truncated,
            );
            if *truncated {
                return;
            }
        }
    }
}
