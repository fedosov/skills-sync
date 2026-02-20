#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use skillssync_core::{
    ScopeFilter, SkillLifecycleStatus, SkillLocator, SkillRecord, SyncEngine, SyncState,
    SyncTrigger,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone, Serialize)]
struct SkillDetails {
    skill: SkillRecord,
    main_file_path: String,
    main_file_exists: bool,
    main_file_body_preview: Option<String>,
    main_file_body_preview_truncated: bool,
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
            ScopeFilter::from_str(value).ok_or_else(|| format!("unsupported scope: {value}"))
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
    let main_file_exists = main_file.exists();
    let (main_file_body_preview, main_file_body_preview_truncated) = read_preview(&main_file, 4000);
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
