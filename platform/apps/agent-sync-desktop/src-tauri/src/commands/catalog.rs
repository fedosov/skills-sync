use agent_sync_core::{SkillLifecycleStatus, SyncEngine, SyncState};

use crate::{
    ensure_write_allowed, find_skill, mutate_catalog_item_inner, CatalogMutationActionPayload,
    CatalogMutationRequestPayload, CatalogMutationTargetPayload, IntoTauriResult, RuntimeState,
};

#[tauri::command]
pub fn mutate_catalog_item(
    request: CatalogMutationRequestPayload,
    runtime: tauri::State<RuntimeState>,
) -> Result<SyncState, String> {
    let engine = SyncEngine::current();
    mutate_catalog_item_inner(request, "mutate_catalog_item", &runtime, &engine)
}

#[tauri::command]
pub fn delete_skill(
    skill_key: String,
    confirmed: bool,
    runtime: tauri::State<RuntimeState>,
) -> Result<SyncState, String> {
    mutate_catalog_item_inner(
        CatalogMutationRequestPayload {
            action: CatalogMutationActionPayload::Delete,
            target: CatalogMutationTargetPayload::Skill { skill_key },
            confirmed,
        },
        "delete_skill",
        &runtime,
        &SyncEngine::current(),
    )
}

#[tauri::command]
pub fn archive_skill(
    skill_key: String,
    confirmed: bool,
    runtime: tauri::State<RuntimeState>,
) -> Result<SyncState, String> {
    mutate_catalog_item_inner(
        CatalogMutationRequestPayload {
            action: CatalogMutationActionPayload::Archive,
            target: CatalogMutationTargetPayload::Skill { skill_key },
            confirmed,
        },
        "archive_skill",
        &runtime,
        &SyncEngine::current(),
    )
}

#[tauri::command]
pub fn restore_skill(
    skill_key: String,
    confirmed: bool,
    runtime: tauri::State<RuntimeState>,
) -> Result<SyncState, String> {
    mutate_catalog_item_inner(
        CatalogMutationRequestPayload {
            action: CatalogMutationActionPayload::Restore,
            target: CatalogMutationTargetPayload::Skill { skill_key },
            confirmed,
        },
        "restore_skill",
        &runtime,
        &SyncEngine::current(),
    )
}

#[tauri::command]
pub fn make_global(
    skill_key: String,
    confirmed: bool,
    runtime: tauri::State<RuntimeState>,
) -> Result<SyncState, String> {
    let engine = SyncEngine::current();
    ensure_write_allowed(&engine, "make_global")?;
    let _guard = runtime.acquire_sync_lock()?;
    let skill = find_skill(&engine, &skill_key, Some(SkillLifecycleStatus::Active))?;
    engine.make_global(&skill, confirmed).to_tauri()
}

#[tauri::command]
pub fn rename_skill(
    skill_key: String,
    new_title: String,
    runtime: tauri::State<RuntimeState>,
) -> Result<SyncState, String> {
    let engine = SyncEngine::current();
    ensure_write_allowed(&engine, "rename_skill")?;
    let _guard = runtime.acquire_sync_lock()?;
    let skill = find_skill(&engine, &skill_key, Some(SkillLifecycleStatus::Active))?;
    engine.rename(&skill, &new_title).to_tauri()
}
