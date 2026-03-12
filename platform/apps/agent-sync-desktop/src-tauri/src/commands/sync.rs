use agent_sync_core::{
    AgentsContextReport, ConfigValidationResult, McpAgent, SubagentRecord, SyncState, SyncTrigger,
};

use crate::{
    app_runtime::AppRuntime,
    command_support::{
        current_engine, ensure_write_allowed, parse_scope_filter, run_sync_with_lock,
        with_locked_write_engine, IntoTauriResult,
    },
};

#[tauri::command]
pub fn run_sync(
    trigger: Option<String>,
    runtime: tauri::State<AppRuntime>,
) -> Result<SyncState, String> {
    let engine = current_engine();
    ensure_write_allowed(&engine, "run_sync")?;
    let parsed = trigger
        .as_deref()
        .map(SyncTrigger::try_from)
        .transpose()
        .to_tauri()?
        .unwrap_or(SyncTrigger::Manual);

    run_sync_with_lock(runtime.inner(), parsed)
}

#[tauri::command]
pub fn get_state() -> SyncState {
    current_engine().load_state()
}

#[tauri::command]
pub fn get_agents_context_report() -> AgentsContextReport {
    current_engine().get_agents_context_report()
}

#[tauri::command]
pub fn get_starred_skill_ids() -> Vec<String> {
    current_engine().starred_skill_ids()
}

#[tauri::command]
pub fn set_skill_starred(skill_id: String, starred: bool) -> Result<Vec<String>, String> {
    let engine = current_engine();
    let state = engine.load_state();
    if !state.skills.iter().any(|skill| skill.id == skill_id) {
        return Err(format!("skill id not found: {skill_id}"));
    }

    engine.set_skill_starred(&skill_id, starred).to_tauri()
}

#[tauri::command]
pub fn list_subagents(scope: Option<String>) -> Result<Vec<SubagentRecord>, String> {
    let engine = current_engine();
    let scope_filter = parse_scope_filter(scope.as_deref())?;
    Ok(engine.list_subagents(scope_filter))
}

#[tauri::command]
pub fn set_mcp_server_enabled(
    server_key: String,
    agent: String,
    enabled: bool,
    scope: Option<String>,
    workspace: Option<String>,
    runtime: tauri::State<AppRuntime>,
) -> Result<SyncState, String> {
    let parsed = agent.parse::<McpAgent>().to_tauri()?;
    with_locked_write_engine(runtime.inner(), "set_mcp_server_enabled", |engine| {
        engine
            .set_mcp_server_enabled(
                &server_key,
                parsed,
                enabled,
                scope.as_deref(),
                workspace.as_deref(),
            )
            .to_tauri()
    })
}

#[tauri::command]
pub fn fix_sync_warning(warning: String, runtime: tauri::State<AppRuntime>) -> Result<(), String> {
    with_locked_write_engine(runtime.inner(), "fix_sync_warning", |engine| {
        engine.fix_sync_warning(&warning).to_tauri()?;
        Ok(())
    })
}

#[tauri::command]
pub fn delete_unmanaged_mcp(
    server_key: String,
    runtime: tauri::State<AppRuntime>,
) -> Result<SyncState, String> {
    with_locked_write_engine(runtime.inner(), "delete_unmanaged_mcp", |engine| {
        engine.delete_unmanaged_mcp(&server_key).to_tauri()
    })
}

#[tauri::command]
pub fn validate_configs() -> Vec<ConfigValidationResult> {
    let engine = current_engine();
    engine.validate_configs()
}
