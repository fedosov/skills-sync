use agent_sync_core::{SkillLifecycleStatus, SyncEngine, SyncState};

use crate::{
    ensure_write_allowed, find_skill, mutate_catalog_item_inner, CatalogMutationRequestPayload,
    IntoTauriResult, RenameSkillResponse, RuntimeState,
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
pub fn rename_skill(
    skill_key: String,
    new_title: String,
    runtime: tauri::State<RuntimeState>,
) -> Result<RenameSkillResponse, String> {
    let engine = SyncEngine::current();
    ensure_write_allowed(&engine, "rename_skill")?;
    let _guard = runtime.acquire_sync_lock()?;
    let skill = find_skill(&engine, &skill_key, Some(SkillLifecycleStatus::Active))?;
    let result = engine.rename(&skill, &new_title).to_tauri()?;
    Ok(RenameSkillResponse {
        state: result.state,
        renamed_skill_key: result.renamed_skill_key,
    })
}
