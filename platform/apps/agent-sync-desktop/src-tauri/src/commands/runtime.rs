use agent_sync_core::AuditEvent;

use crate::{
    app_runtime::{AppRuntime, RuntimeControls},
    command_support::{current_engine, parse_audit_status, with_locked_engine, IntoTauriResult},
};

#[tauri::command]
pub fn get_runtime_controls(runtime: tauri::State<AppRuntime>) -> RuntimeControls {
    let engine = current_engine();
    runtime.inner().runtime_controls(&engine)
}

#[tauri::command]
pub fn set_allow_filesystem_changes(
    allow: bool,
    runtime: tauri::State<AppRuntime>,
) -> Result<RuntimeControls, String> {
    let engine = current_engine();
    runtime.inner().set_allow_filesystem_changes(&engine, allow)
}

#[tauri::command]
pub fn list_audit_events(
    limit: Option<usize>,
    status: Option<String>,
    action: Option<String>,
) -> Result<Vec<AuditEvent>, String> {
    let parsed_status = parse_audit_status(status.as_deref())?;
    let events = current_engine().list_audit_events(limit, parsed_status, action.as_deref());
    Ok(events)
}

#[tauri::command]
pub fn clear_audit_events(runtime: tauri::State<AppRuntime>) -> Result<(), String> {
    with_locked_engine(runtime.inner(), |engine| {
        engine.clear_audit_events().to_tauri()
    })
}
