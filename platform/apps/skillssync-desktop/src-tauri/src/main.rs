#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use skillssync_core::{
    ScopeFilter, SkillLifecycleStatus, SkillLocator, SkillRecord, SyncEngine, SyncState,
    SyncTrigger,
};
use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;

const MAX_MAIN_FILE_PREVIEW_CHARS: usize = usize::MAX;
const MAX_TREE_ENTRIES: usize = usize::MAX;

#[derive(Debug, Clone, Serialize)]
struct SkillDetails {
    skill: SkillRecord,
    main_file_path: String,
    main_file_exists: bool,
    main_file_body_preview: Option<String>,
    main_file_body_preview_truncated: bool,
    skill_dir_tree_preview: Option<String>,
    skill_dir_tree_preview_truncated: bool,
    last_modified_unix_seconds: Option<u64>,
}

#[tauri::command]
fn run_sync(trigger: Option<String>) -> Result<SyncState, String> {
    let engine = SyncEngine::current();
    let parsed = trigger
        .as_deref()
        .map(SyncTrigger::try_from)
        .transpose()
        .map_err(|error| error.to_string())?
        .unwrap_or(SyncTrigger::Manual);

    engine.run_sync(parsed).map_err(|error| error.to_string())
}

#[tauri::command]
fn get_state() -> SyncState {
    SyncEngine::current().load_state()
}

#[tauri::command]
fn list_skills(scope: Option<String>) -> Result<Vec<SkillRecord>, String> {
    let engine = SyncEngine::current();
    let scope_filter = scope
        .as_deref()
        .map(|value| {
            value
                .parse::<ScopeFilter>()
                .map_err(|_| format!("unsupported scope: {value}"))
        })
        .transpose()?
        .unwrap_or(ScopeFilter::All);
    Ok(engine.list_skills(scope_filter))
}

#[tauri::command]
fn delete_skill(skill_key: String, confirmed: bool) -> Result<SyncState, String> {
    let engine = SyncEngine::current();
    let skill = find_skill(&engine, &skill_key, None)?;
    engine
        .delete(&skill, confirmed)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn archive_skill(skill_key: String, confirmed: bool) -> Result<SyncState, String> {
    let engine = SyncEngine::current();
    let skill = find_skill(&engine, &skill_key, Some(SkillLifecycleStatus::Active))?;
    engine
        .archive(&skill, confirmed)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn restore_skill(skill_key: String, confirmed: bool) -> Result<SyncState, String> {
    let engine = SyncEngine::current();
    let skill = find_skill(&engine, &skill_key, Some(SkillLifecycleStatus::Archived))?;
    engine
        .restore(&skill, confirmed)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn make_global(skill_key: String, confirmed: bool) -> Result<SyncState, String> {
    let engine = SyncEngine::current();
    let skill = find_skill(&engine, &skill_key, Some(SkillLifecycleStatus::Active))?;
    engine
        .make_global(&skill, confirmed)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn rename_skill(skill_key: String, new_title: String) -> Result<SyncState, String> {
    let engine = SyncEngine::current();
    let skill = find_skill(&engine, &skill_key, Some(SkillLifecycleStatus::Active))?;
    engine
        .rename(&skill, &new_title)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_skill_details(skill_key: String) -> Result<SkillDetails, String> {
    let engine = SyncEngine::current();
    let skill = find_skill(&engine, &skill_key, None)?;
    let main_file = resolve_main_skill_file(&skill);
    let skill_root = resolve_skill_root_dir(&skill, &main_file);
    let main_file_exists = main_file.exists();
    let (main_file_body_preview, main_file_body_preview_truncated) =
        read_preview(&main_file, MAX_MAIN_FILE_PREVIEW_CHARS);
    let (skill_dir_tree_preview, skill_dir_tree_preview_truncated) =
        read_skill_dir_tree(&skill_root, MAX_TREE_ENTRIES);
    let last_modified_unix_seconds = fs::metadata(&main_file)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|ts| ts.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs());

    Ok(SkillDetails {
        skill,
        main_file_path: main_file.display().to_string(),
        main_file_exists,
        main_file_body_preview,
        main_file_body_preview_truncated,
        skill_dir_tree_preview,
        skill_dir_tree_preview_truncated,
        last_modified_unix_seconds,
    })
}

#[tauri::command]
fn open_skill_path(skill_key: String, target: Option<String>) -> Result<(), String> {
    let engine = SyncEngine::current();
    let skill = find_skill(&engine, &skill_key, None)?;
    let selected_target = target.unwrap_or_else(|| String::from("folder"));
    let path = match selected_target.as_str() {
        "folder" => PathBuf::from(&skill.canonical_source_path),
        "file" => resolve_main_skill_file(&skill),
        other => {
            return Err(format!(
                "unsupported target: {other} (allowed: folder|file)"
            ));
        }
    };
    open_path(&path)
}

fn find_skill(
    engine: &SyncEngine,
    skill_key: &str,
    status: Option<SkillLifecycleStatus>,
) -> Result<SkillRecord, String> {
    engine
        .find_skill(&SkillLocator {
            skill_key: skill_key.to_owned(),
            status,
        })
        .ok_or_else(|| format!("skill not found: {skill_key}"))
}

fn resolve_main_skill_file(skill: &SkillRecord) -> PathBuf {
    let source = PathBuf::from(&skill.canonical_source_path);
    if skill.package_type == "dir" {
        source.join("SKILL.md")
    } else {
        source
    }
}

fn resolve_skill_root_dir(skill: &SkillRecord, main_file: &Path) -> PathBuf {
    if skill.package_type == "dir" {
        return PathBuf::from(&skill.canonical_source_path);
    }
    main_file
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(&skill.canonical_source_path))
}

fn read_preview(path: &Path, max_chars: usize) -> (Option<String>, bool) {
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

fn read_skill_dir_tree(root: &Path, max_entries: usize) -> (Option<String>, bool) {
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

fn open_path(path: &Path) -> Result<(), String> {
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

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            run_sync,
            get_state,
            list_skills,
            delete_skill,
            archive_skill,
            restore_skill,
            make_global,
            rename_skill,
            get_skill_details,
            open_skill_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running skillssync desktop app");
}

#[cfg(test)]
mod tests {
    use super::read_skill_dir_tree;
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn read_skill_dir_tree_returns_stable_ascii_structure() {
        let tempdir = tempdir().expect("create temp dir");
        let root = tempdir.path().join("alpha-skill");
        fs::create_dir_all(root.join("references")).expect("create nested dir");
        fs::write(root.join("SKILL.md"), "# Skill").expect("write SKILL.md");
        fs::write(root.join("README.md"), "readme").expect("write README.md");
        fs::write(root.join("references").join("notes.md"), "notes").expect("write nested file");

        let (tree, truncated) = read_skill_dir_tree(Path::new(&root), 1000);

        assert!(!truncated);
        assert_eq!(
            tree,
            Some(String::from(
                "alpha-skill/\n|-- references/\n|   `-- notes.md\n|-- README.md\n`-- SKILL.md"
            ))
        );
    }

    #[test]
    fn read_skill_dir_tree_marks_truncation_when_limit_reached() {
        let tempdir = tempdir().expect("create temp dir");
        let root = tempdir.path().join("beta-skill");
        fs::create_dir_all(&root).expect("create root");
        fs::write(root.join("a.md"), "a").expect("write a");
        fs::write(root.join("b.md"), "b").expect("write b");
        fs::write(root.join("c.md"), "c").expect("write c");

        let (tree, truncated) = read_skill_dir_tree(Path::new(&root), 2);

        assert!(truncated);
        assert_eq!(tree, Some(String::from("beta-skill/\n|-- a.md\n|-- b.md")));
    }
}
