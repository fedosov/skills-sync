use agent_sync_core::{
    AuditEventStatus, CatalogMutationAction, DotagentsScope, ScopeFilter, SyncEngine, SyncState,
    SyncTrigger,
};

use crate::{
    app_runtime::AppRuntime,
    catalog_support::{
        to_catalog_mutation_target, validate_catalog_mutation_target, CatalogMutationRequestPayload,
    },
};

pub(crate) trait IntoTauriResult<T> {
    fn to_tauri(self) -> Result<T, String>;
}

impl<T, E: std::fmt::Display> IntoTauriResult<T> for Result<T, E> {
    fn to_tauri(self) -> Result<T, String> {
        self.map_err(|e| e.to_string())
    }
}

pub(crate) fn current_engine() -> SyncEngine {
    SyncEngine::current()
}

pub(crate) fn ensure_write_allowed(engine: &SyncEngine, action: &str) -> Result<(), String> {
    if engine.allow_filesystem_changes() {
        return Ok(());
    }
    Err(format!(
        "Filesystem changes are disabled. Enable 'Allow filesystem changes' to run {action}."
    ))
}

pub(crate) fn parse_audit_status(value: Option<&str>) -> Result<Option<AuditEventStatus>, String> {
    let Some(raw) = value else {
        return Ok(None);
    };
    match raw.trim().to_ascii_lowercase().as_str() {
        "success" => Ok(Some(AuditEventStatus::Success)),
        "failed" => Ok(Some(AuditEventStatus::Failed)),
        "blocked" => Ok(Some(AuditEventStatus::Blocked)),
        other => Err(format!(
            "unsupported audit status: {other} (success|failed|blocked)"
        )),
    }
}

pub(crate) fn parse_scope_filter(scope: Option<&str>) -> Result<ScopeFilter, String> {
    let Some(value) = scope else {
        return Ok(ScopeFilter::All);
    };
    value
        .parse::<ScopeFilter>()
        .map_err(|_| format!("unsupported scope: {value}"))
}

pub(crate) fn parse_dotagents_scope(value: Option<&str>) -> Result<DotagentsScope, String> {
    let normalized = value.unwrap_or("all");
    normalized
        .parse::<DotagentsScope>()
        .map_err(|_| format!("unsupported scope: {normalized} (all|user|project)"))
}

pub(crate) fn with_locked_engine<T, F>(runtime: &AppRuntime, operation: F) -> Result<T, String>
where
    F: FnOnce(&SyncEngine) -> Result<T, String>,
{
    let engine = current_engine();
    runtime.with_sync_lock(|| operation(&engine))
}

pub(crate) fn with_locked_write_engine<T, F>(
    runtime: &AppRuntime,
    action: &str,
    operation: F,
) -> Result<T, String>
where
    F: FnOnce(&SyncEngine) -> Result<T, String>,
{
    let engine = current_engine();
    ensure_write_allowed(&engine, action)?;
    runtime.with_sync_lock(|| operation(&engine))
}

pub(crate) fn run_sync_with_lock(
    runtime: &AppRuntime,
    trigger: SyncTrigger,
) -> Result<SyncState, String> {
    with_locked_engine(runtime, |engine| engine.run_sync(trigger).to_tauri())
}

pub(crate) fn mutate_catalog_item_inner(
    request: CatalogMutationRequestPayload,
    action_name: &str,
    runtime: &AppRuntime,
) -> Result<SyncState, String> {
    validate_catalog_mutation_target(&request.target)?;
    with_locked_write_engine(runtime, action_name, |engine| {
        let action = CatalogMutationAction::from(request.action);
        let target = to_catalog_mutation_target(request.target)?;
        engine
            .mutate_catalog_item(action, target, request.confirmed)
            .to_tauri()
    })
}
