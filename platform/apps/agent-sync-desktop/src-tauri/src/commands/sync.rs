use agent_sync_core::{
    AgentsContextReport, McpAgent, McpServerRecord, SubagentRecord, SyncEngine, SyncState,
    SyncTrigger,
};

use crate::{
    ensure_write_allowed, parse_scope_filter, run_sync_with_lock, IntoTauriResult, RuntimeState,
};

#[tauri::command]
pub fn run_sync(
    trigger: Option<String>,
    runtime: tauri::State<RuntimeState>,
) -> Result<SyncState, String> {
    let engine = SyncEngine::current();
    ensure_write_allowed(&engine, "run_sync")?;
    let parsed = trigger
        .as_deref()
        .map(SyncTrigger::try_from)
        .transpose()
        .to_tauri()?
        .unwrap_or(SyncTrigger::Manual);

    run_sync_with_lock(&engine, &runtime, parsed)
}

#[tauri::command]
pub fn get_state() -> SyncState {
    SyncEngine::current().load_state()
}

#[tauri::command]
pub fn get_agents_context_report() -> AgentsContextReport {
    SyncEngine::current().get_agents_context_report()
}

#[tauri::command]
pub fn get_starred_skill_ids() -> Vec<String> {
    SyncEngine::current().starred_skill_ids()
}

#[tauri::command]
pub fn set_skill_starred(skill_id: String, starred: bool) -> Result<Vec<String>, String> {
    let engine = SyncEngine::current();
    let state = engine.load_state();
    if !state.skills.iter().any(|skill| skill.id == skill_id) {
        return Err(format!("skill id not found: {skill_id}"));
    }

    engine.set_skill_starred(&skill_id, starred).to_tauri()
}

#[tauri::command]
pub fn list_subagents(scope: Option<String>) -> Result<Vec<SubagentRecord>, String> {
    let engine = SyncEngine::current();
    let scope_filter = parse_scope_filter(scope.as_deref())?;
    Ok(engine.list_subagents(scope_filter))
}

#[tauri::command]
pub fn get_mcp_servers() -> Vec<McpServerRecord> {
    SyncEngine::current().list_mcp_servers()
}

#[tauri::command]
pub fn set_mcp_server_enabled(
    server_key: String,
    agent: String,
    enabled: bool,
    scope: Option<String>,
    workspace: Option<String>,
    runtime: tauri::State<RuntimeState>,
) -> Result<SyncState, String> {
    let engine = SyncEngine::current();
    ensure_write_allowed(&engine, "set_mcp_server_enabled")?;
    let parsed = agent.parse::<McpAgent>().to_tauri()?;
    let _guard = runtime.acquire_sync_lock()?;
    engine
        .set_mcp_server_enabled(
            &server_key,
            parsed,
            enabled,
            scope.as_deref(),
            workspace.as_deref(),
        )
        .to_tauri()
}

#[tauri::command]
pub fn fix_sync_warning(
    warning: String,
    runtime: tauri::State<RuntimeState>,
) -> Result<(), String> {
    let engine = SyncEngine::current();
    ensure_write_allowed(&engine, "fix_sync_warning")?;
    let _guard = runtime.acquire_sync_lock()?;
    engine.fix_sync_warning(&warning).to_tauri()?;
    Ok(())
}
