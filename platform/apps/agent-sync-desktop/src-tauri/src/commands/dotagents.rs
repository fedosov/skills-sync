use agent_sync_core::{McpServerRecord, SkillRecord};

use crate::{
    app_runtime::AppRuntime,
    command_support::{
        current_engine, parse_dotagents_scope, with_locked_write_engine, IntoTauriResult,
    },
};

#[tauri::command]
pub fn run_dotagents_sync(
    scope: Option<String>,
    runtime: tauri::State<AppRuntime>,
) -> Result<(), String> {
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    with_locked_write_engine(runtime.inner(), "run_dotagents_sync", |engine| {
        engine.run_dotagents_sync(parsed_scope).to_tauri()?;
        engine
            .run_dotagents_install_frozen(parsed_scope)
            .to_tauri()?;
        Ok(())
    })
}

#[tauri::command]
pub fn list_dotagents_skills(scope: Option<String>) -> Result<Vec<SkillRecord>, String> {
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    current_engine()
        .list_dotagents_skills(parsed_scope)
        .to_tauri()
}

#[tauri::command]
pub fn list_dotagents_mcp(scope: Option<String>) -> Result<Vec<McpServerRecord>, String> {
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    current_engine().list_dotagents_mcp(parsed_scope).to_tauri()
}

#[tauri::command]
pub fn dotagents_skills_install(
    scope: Option<String>,
    runtime: tauri::State<AppRuntime>,
) -> Result<(), String> {
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    with_locked_write_engine(runtime.inner(), "dotagents_skills_install", |engine| {
        engine.run_dotagents_install_frozen(parsed_scope).to_tauri()
    })
}

#[tauri::command]
pub fn dotagents_skills_add(
    package: String,
    scope: Option<String>,
    runtime: tauri::State<AppRuntime>,
) -> Result<(), String> {
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    with_locked_write_engine(runtime.inner(), "dotagents_skills_add", |engine| {
        engine
            .run_dotagents_command(parsed_scope, &["add", package.as_str()])
            .to_tauri()
    })
}

#[tauri::command]
pub fn dotagents_skills_remove(
    package: String,
    scope: Option<String>,
    runtime: tauri::State<AppRuntime>,
) -> Result<(), String> {
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    with_locked_write_engine(runtime.inner(), "dotagents_skills_remove", |engine| {
        engine
            .run_dotagents_command(parsed_scope, &["remove", package.as_str()])
            .to_tauri()
    })
}

#[tauri::command]
pub fn dotagents_skills_update(
    package: Option<String>,
    scope: Option<String>,
    runtime: tauri::State<AppRuntime>,
) -> Result<(), String> {
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    let mut command = vec![String::from("update")];
    if let Some(pkg) = package {
        command.push(pkg);
    }
    let refs = command.iter().map(String::as_str).collect::<Vec<_>>();
    with_locked_write_engine(runtime.inner(), "dotagents_skills_update", |engine| {
        engine.run_dotagents_command(parsed_scope, &refs).to_tauri()
    })
}

#[tauri::command]
pub fn dotagents_mcp_add(
    args: Vec<String>,
    scope: Option<String>,
    runtime: tauri::State<AppRuntime>,
) -> Result<(), String> {
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    let mut command = vec![String::from("mcp"), String::from("add")];
    command.extend(args);
    let refs = command.iter().map(String::as_str).collect::<Vec<_>>();
    with_locked_write_engine(runtime.inner(), "dotagents_mcp_add", |engine| {
        engine.run_dotagents_command(parsed_scope, &refs).to_tauri()
    })
}

#[tauri::command]
pub fn dotagents_mcp_remove(
    args: Vec<String>,
    scope: Option<String>,
    runtime: tauri::State<AppRuntime>,
) -> Result<(), String> {
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    let mut command = vec![String::from("mcp"), String::from("remove")];
    command.extend(args);
    let refs = command.iter().map(String::as_str).collect::<Vec<_>>();
    with_locked_write_engine(runtime.inner(), "dotagents_mcp_remove", |engine| {
        engine.run_dotagents_command(parsed_scope, &refs).to_tauri()
    })
}

#[tauri::command]
pub fn migrate_dotagents(
    scope: Option<String>,
    runtime: tauri::State<AppRuntime>,
) -> Result<(), String> {
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    with_locked_write_engine(runtime.inner(), "migrate_dotagents", |engine| {
        engine.migrate_to_dotagents(parsed_scope).to_tauri()
    })
}
