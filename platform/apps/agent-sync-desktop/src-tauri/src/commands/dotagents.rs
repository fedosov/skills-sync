use agent_sync_core::{McpServerRecord, SkillRecord, SyncEngine};

use crate::{ensure_write_allowed, parse_dotagents_scope, IntoTauriResult, RuntimeState};

#[tauri::command]
pub fn run_dotagents_sync(
    scope: Option<String>,
    runtime: tauri::State<RuntimeState>,
) -> Result<(), String> {
    let engine = SyncEngine::current();
    ensure_write_allowed(&engine, "run_dotagents_sync")?;
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    let _guard = runtime.acquire_sync_lock()?;
    engine.run_dotagents_sync(parsed_scope).to_tauri()?;
    engine
        .run_dotagents_install_frozen(parsed_scope)
        .to_tauri()?;
    Ok(())
}

#[tauri::command]
pub fn list_dotagents_skills(scope: Option<String>) -> Result<Vec<SkillRecord>, String> {
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    SyncEngine::current()
        .list_dotagents_skills(parsed_scope)
        .to_tauri()
}

#[tauri::command]
pub fn list_dotagents_mcp(scope: Option<String>) -> Result<Vec<McpServerRecord>, String> {
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    SyncEngine::current()
        .list_dotagents_mcp(parsed_scope)
        .to_tauri()
}

#[tauri::command]
pub fn dotagents_skills_install(
    scope: Option<String>,
    runtime: tauri::State<RuntimeState>,
) -> Result<(), String> {
    let engine = SyncEngine::current();
    ensure_write_allowed(&engine, "dotagents_skills_install")?;
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    let _guard = runtime.acquire_sync_lock()?;
    engine.run_dotagents_install_frozen(parsed_scope).to_tauri()
}

#[tauri::command]
pub fn dotagents_skills_add(
    package: String,
    scope: Option<String>,
    runtime: tauri::State<RuntimeState>,
) -> Result<(), String> {
    let engine = SyncEngine::current();
    ensure_write_allowed(&engine, "dotagents_skills_add")?;
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    let _guard = runtime.acquire_sync_lock()?;
    engine
        .run_dotagents_command(parsed_scope, &["add", package.as_str()])
        .to_tauri()
}

#[tauri::command]
pub fn dotagents_skills_remove(
    package: String,
    scope: Option<String>,
    runtime: tauri::State<RuntimeState>,
) -> Result<(), String> {
    let engine = SyncEngine::current();
    ensure_write_allowed(&engine, "dotagents_skills_remove")?;
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    let _guard = runtime.acquire_sync_lock()?;
    engine
        .run_dotagents_command(parsed_scope, &["remove", package.as_str()])
        .to_tauri()
}

#[tauri::command]
pub fn dotagents_skills_update(
    package: Option<String>,
    scope: Option<String>,
    runtime: tauri::State<RuntimeState>,
) -> Result<(), String> {
    let engine = SyncEngine::current();
    ensure_write_allowed(&engine, "dotagents_skills_update")?;
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    let _guard = runtime.acquire_sync_lock()?;

    let mut command = vec![String::from("update")];
    if let Some(pkg) = package {
        command.push(pkg);
    }
    let refs = command.iter().map(String::as_str).collect::<Vec<_>>();
    engine.run_dotagents_command(parsed_scope, &refs).to_tauri()
}

#[tauri::command]
pub fn dotagents_mcp_add(
    args: Vec<String>,
    scope: Option<String>,
    runtime: tauri::State<RuntimeState>,
) -> Result<(), String> {
    let engine = SyncEngine::current();
    ensure_write_allowed(&engine, "dotagents_mcp_add")?;
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    let _guard = runtime.acquire_sync_lock()?;
    let mut command = vec![String::from("mcp"), String::from("add")];
    command.extend(args);
    let refs = command.iter().map(String::as_str).collect::<Vec<_>>();
    engine.run_dotagents_command(parsed_scope, &refs).to_tauri()
}

#[tauri::command]
pub fn dotagents_mcp_remove(
    args: Vec<String>,
    scope: Option<String>,
    runtime: tauri::State<RuntimeState>,
) -> Result<(), String> {
    let engine = SyncEngine::current();
    ensure_write_allowed(&engine, "dotagents_mcp_remove")?;
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    let _guard = runtime.acquire_sync_lock()?;
    let mut command = vec![String::from("mcp"), String::from("remove")];
    command.extend(args);
    let refs = command.iter().map(String::as_str).collect::<Vec<_>>();
    engine.run_dotagents_command(parsed_scope, &refs).to_tauri()
}

#[tauri::command]
pub fn migrate_dotagents(
    scope: Option<String>,
    runtime: tauri::State<RuntimeState>,
) -> Result<(), String> {
    let engine = SyncEngine::current();
    ensure_write_allowed(&engine, "migrate_dotagents")?;
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    let _guard = runtime.acquire_sync_lock()?;
    engine.migrate_to_dotagents(parsed_scope).to_tauri()
}
