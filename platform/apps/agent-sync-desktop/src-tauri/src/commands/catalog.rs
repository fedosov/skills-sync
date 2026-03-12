use agent_sync_core::{SkillLifecycleStatus, SyncState};

use crate::{
    app_runtime::AppRuntime,
    catalog_support::{find_skill, CatalogMutationRequestPayload, RenameSkillResponse},
    command_support::{mutate_catalog_item_inner, with_locked_write_engine, IntoTauriResult},
};

#[tauri::command]
pub fn mutate_catalog_item(
    request: CatalogMutationRequestPayload,
    runtime: tauri::State<AppRuntime>,
) -> Result<SyncState, String> {
    mutate_catalog_item_inner(request, "mutate_catalog_item", runtime.inner())
}

#[tauri::command]
pub fn rename_skill(
    skill_key: String,
    new_title: String,
    runtime: tauri::State<AppRuntime>,
) -> Result<RenameSkillResponse, String> {
    with_locked_write_engine(runtime.inner(), "rename_skill", |engine| {
        let skill = find_skill(engine, &skill_key, Some(SkillLifecycleStatus::Active))?;
        let result = engine.rename(&skill, &new_title).to_tauri()?;
        Ok(RenameSkillResponse {
            state: result.state,
            renamed_skill_key: result.renamed_skill_key,
        })
    })
}
