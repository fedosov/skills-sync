use agent_sync_core::{AuditEvent, SyncEngine};

use crate::{
    parse_audit_status, runtime_controls, set_allow_filesystem_changes_inner, IntoTauriResult,
    RuntimeControls, RuntimeState,
};

#[tauri::command]
pub fn get_runtime_controls(runtime: tauri::State<RuntimeState>) -> RuntimeControls {
    runtime_controls(&SyncEngine::current(), &runtime)
}

#[tauri::command]
pub fn set_allow_filesystem_changes(
    allow: bool,
    runtime: tauri::State<RuntimeState>,
) -> Result<RuntimeControls, String> {
    let engine = SyncEngine::current();
    set_allow_filesystem_changes_inner(allow, &runtime, &engine)
}

#[tauri::command]
pub fn list_audit_events(
    limit: Option<usize>,
    status: Option<String>,
    action: Option<String>,
) -> Result<Vec<AuditEvent>, String> {
    let parsed_status = parse_audit_status(status.as_deref())?;
    let events = SyncEngine::current().list_audit_events(limit, parsed_status, action.as_deref());
    Ok(events)
}

#[tauri::command]
pub fn clear_audit_events(runtime: tauri::State<RuntimeState>) -> Result<(), String> {
    let engine = SyncEngine::current();
    let _guard = runtime.acquire_sync_lock()?;
    engine.clear_audit_events().to_tauri()
}
