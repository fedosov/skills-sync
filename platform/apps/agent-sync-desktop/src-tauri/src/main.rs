#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_state;
mod dotagents_runner;
mod dotagents_runtime;
mod open_path;
mod settings;

use app_state::{AppContext, AppState};
use dotagents_runner::{
    DotagentsCommandRequest, DotagentsCommandResult, DotagentsMcpListItem, DotagentsSkillListItem,
};
use dotagents_runtime::{DotagentsRuntimeManager, DotagentsRuntimeStatus};
use settings::DotagentsScope;
use tauri::Manager;

fn build_app_state<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<AppState, String> {
    let home_dir = app
        .path()
        .home_dir()
        .map_err(|error| format!("failed to resolve user home directory: {error}"))?;
    let settings_dir = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("failed to resolve app config directory: {error}"))?;
    let runner =
        dotagents_runner::DotagentsRunner::new(home_dir.clone(), DotagentsRuntimeManager::new());
    Ok(AppState::new(home_dir, settings_dir, runner))
}

#[tauri::command]
fn get_runtime_status(state: tauri::State<AppState>) -> DotagentsRuntimeStatus {
    state.get_runtime_status()
}

#[tauri::command]
fn get_app_context(state: tauri::State<AppState>) -> Result<AppContext, String> {
    state.get_app_context()
}

#[tauri::command]
fn set_scope(scope: DotagentsScope, state: tauri::State<AppState>) -> Result<AppContext, String> {
    state.set_scope(scope)
}

#[tauri::command]
fn set_project_root(
    project_root: Option<String>,
    state: tauri::State<AppState>,
) -> Result<AppContext, String> {
    state.set_project_root(project_root)
}

#[tauri::command]
fn list_skills(state: tauri::State<AppState>) -> Result<Vec<DotagentsSkillListItem>, String> {
    state.list_skills()
}

#[tauri::command]
fn list_mcp_servers(state: tauri::State<AppState>) -> Result<Vec<DotagentsMcpListItem>, String> {
    state.list_mcp_servers()
}

#[tauri::command]
fn run_dotagents_command(
    request: DotagentsCommandRequest,
    state: tauri::State<AppState>,
) -> Result<DotagentsCommandResult, String> {
    state.run_dotagents_command(request)
}

#[tauri::command]
fn open_agents_toml(state: tauri::State<AppState>) -> Result<(), String> {
    state.open_agents_toml()
}

#[tauri::command]
fn open_agents_dir(state: tauri::State<AppState>) -> Result<(), String> {
    state.open_agents_dir()
}

#[tauri::command]
fn open_user_home(state: tauri::State<AppState>) -> Result<(), String> {
    state.open_user_home()
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let state = build_app_state(app.handle())?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_runtime_status,
            get_app_context,
            set_scope,
            set_project_root,
            list_skills,
            list_mcp_servers,
            run_dotagents_command,
            open_agents_toml,
            open_agents_dir,
            open_user_home,
        ])
        .run(tauri::generate_context!())
        .expect("error while running dotagents desktop");
}
