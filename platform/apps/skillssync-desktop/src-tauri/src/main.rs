#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use skillssync_core::{
    watch::SyncWatchStream, AuditEvent, AuditEventStatus, DotagentsScope, McpAgent,
    McpServerRecord, ScopeFilter, SkillLifecycleStatus, SkillLocator, SkillRecord, SubagentRecord,
    SyncEngine, SyncState, SyncTrigger,
};
use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, UNIX_EPOCH};
use tauri::Manager;

const MAX_MAIN_FILE_PREVIEW_CHARS: usize = usize::MAX;
const MAX_TREE_ENTRIES: usize = usize::MAX;
const AUTO_WATCH_DEBOUNCE_MS: u64 = 800;

#[derive(Debug, Clone, Serialize)]
struct RuntimeControls {
    allow_filesystem_changes: bool,
    auto_watch_active: bool,
}

#[derive(Debug, Default)]
struct WatchRuntime {
    active: bool,
    stop_tx: Option<mpsc::Sender<()>>,
    handle: Option<JoinHandle<()>>,
}

#[derive(Debug, Clone)]
struct RuntimeState {
    watch: Arc<Mutex<WatchRuntime>>,
    sync_lock: Arc<Mutex<()>>,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            watch: Arc::new(Mutex::new(WatchRuntime::default())),
            sync_lock: Arc::new(Mutex::new(())),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct PlatformContext {
    os: String,
    linux_desktop: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct SkillDetails {
    skill: SkillRecord,
    main_file_path: String,
    main_file_exists: bool,
    main_file_body_preview: Option<String>,
    main_file_body_preview_truncated: bool,
    skill_dir_tree_preview: Option<String>,
    skill_dir_tree_preview_truncated: bool,
    last_modified_unix_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
struct SubagentDetails {
    subagent: SubagentRecord,
    main_file_path: String,
    main_file_exists: bool,
    main_file_body_preview: Option<String>,
    main_file_body_preview_truncated: bool,
    subagent_dir_tree_preview: Option<String>,
    subagent_dir_tree_preview_truncated: bool,
    last_modified_unix_seconds: Option<u64>,
    target_statuses: Vec<SubagentTargetStatus>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum SubagentTargetKind {
    Symlink,
    RegularFile,
    Missing,
    Other,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct SubagentTargetStatus {
    path: String,
    exists: bool,
    is_symlink: bool,
    symlink_target: Option<String>,
    points_to_canonical: bool,
    kind: SubagentTargetKind,
}

fn blocked_write_message(action: &str) -> String {
    format!("Filesystem changes are disabled. Enable 'Allow filesystem changes' to run {action}.")
}

fn ensure_write_allowed(engine: &SyncEngine, action: &str) -> Result<(), String> {
    if engine.allow_filesystem_changes() {
        return Ok(());
    }
    Err(blocked_write_message(action))
}

fn parse_audit_status(value: Option<&str>) -> Result<Option<AuditEventStatus>, String> {
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

fn parse_dotagents_scope(value: Option<&str>) -> Result<DotagentsScope, String> {
    let normalized = value.unwrap_or("all");
    normalized
        .parse::<DotagentsScope>()
        .map_err(|_| format!("unsupported scope: {normalized} (all|user|project)"))
}

fn runtime_controls(engine: &SyncEngine, runtime: &RuntimeState) -> RuntimeControls {
    let auto_watch_active = runtime
        .watch
        .lock()
        .map(|state| state.active)
        .unwrap_or(false);
    RuntimeControls {
        allow_filesystem_changes: engine.allow_filesystem_changes(),
        auto_watch_active,
    }
}

fn run_sync_with_lock(
    engine: &SyncEngine,
    runtime: &RuntimeState,
    trigger: SyncTrigger,
) -> Result<SyncState, String> {
    let _guard = runtime
        .sync_lock
        .lock()
        .map_err(|_| String::from("internal lock error"))?;
    engine.run_sync(trigger).map_err(|error| error.to_string())
}

fn enable_auto_watch_and_initial_sync(
    runtime: &RuntimeState,
    engine: &SyncEngine,
) -> Result<(), String> {
    if !engine.allow_filesystem_changes() {
        return Ok(());
    }

    run_sync_with_lock(engine, runtime, SyncTrigger::Manual).map(|_| ())?;

    if !engine.allow_filesystem_changes() {
        return Ok(());
    }

    start_auto_watch(runtime, engine)?;

    if !engine.allow_filesystem_changes() {
        stop_auto_watch(runtime);
    }

    Ok(())
}

fn stop_auto_watch(runtime: &RuntimeState) {
    let mut handle_to_join: Option<JoinHandle<()>> = None;
    if let Ok(mut state) = runtime.watch.lock() {
        if !state.active {
            return;
        }
        if let Some(stop_tx) = state.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        handle_to_join = state.handle.take();
        state.active = false;
    }

    if let Some(handle) = handle_to_join {
        let _ = handle.join();
    }
}

fn start_auto_watch(runtime: &RuntimeState, engine: &SyncEngine) -> Result<(), String> {
    if runtime
        .watch
        .lock()
        .map(|state| state.active)
        .unwrap_or(false)
    {
        return Ok(());
    }

    let watch_paths = engine.watch_paths();
    let stream = SyncWatchStream::new(&watch_paths)
        .map_err(|error| format!("failed to start filesystem watcher: {error}"))?;
    let sync_lock = Arc::clone(&runtime.sync_lock);
    let thread_engine = engine.clone();
    let (stop_tx, stop_rx) = mpsc::channel::<()>();

    let handle = std::thread::spawn(move || {
        let mut pending_since: Option<Instant> = None;

        loop {
            if stop_rx.try_recv().is_ok() {
                break;
            }

            match stream.recv_timeout(Duration::from_millis(250)) {
                Some(Ok(_)) => {
                    pending_since = Some(Instant::now());
                }
                Some(Err(_)) => {}
                None => {}
            }

            let should_sync = pending_since
                .map(|started| started.elapsed() >= Duration::from_millis(AUTO_WATCH_DEBOUNCE_MS))
                .unwrap_or(false);
            if !should_sync {
                continue;
            }

            pending_since = None;
            if let Ok(_guard) = sync_lock.lock() {
                let _ = thread_engine.run_sync(SyncTrigger::AutoFilesystem);
            }
        }
    });

    if let Ok(mut state) = runtime.watch.lock() {
        state.stop_tx = Some(stop_tx);
        state.handle = Some(handle);
        state.active = true;
        return Ok(());
    }

    Err(String::from("failed to update watcher runtime state"))
}

fn set_allow_filesystem_changes_inner_with<F>(
    allow: bool,
    runtime: &RuntimeState,
    engine: &SyncEngine,
    enable_when_allowed: F,
) -> Result<RuntimeControls, String>
where
    F: Fn(&RuntimeState, &SyncEngine) -> Result<(), String>,
{
    engine
        .set_allow_filesystem_changes(allow)
        .map_err(|error| error.to_string())?;

    if allow {
        if let Err(error) = enable_when_allowed(runtime, engine) {
            stop_auto_watch(runtime);
            if let Err(rollback_error) = engine.set_allow_filesystem_changes(false) {
                return Err(format!(
                    "{error}; failed to revert filesystem write mode: {rollback_error}"
                ));
            }
            return Err(error);
        }
    } else {
        stop_auto_watch(runtime);
    }

    Ok(runtime_controls(engine, runtime))
}

fn set_allow_filesystem_changes_inner(
    allow: bool,
    runtime: &RuntimeState,
    engine: &SyncEngine,
) -> Result<RuntimeControls, String> {
    set_allow_filesystem_changes_inner_with(
        allow,
        runtime,
        engine,
        enable_auto_watch_and_initial_sync,
    )
}

#[tauri::command]
fn run_sync(
    trigger: Option<String>,
    runtime: tauri::State<RuntimeState>,
) -> Result<SyncState, String> {
    let engine = SyncEngine::current();
    ensure_write_allowed(&engine, "run_sync")?;
    let parsed = trigger
        .as_deref()
        .map(SyncTrigger::try_from)
        .transpose()
        .map_err(|error| error.to_string())?
        .unwrap_or(SyncTrigger::Manual);

    run_sync_with_lock(&engine, &runtime, parsed)
}

#[tauri::command]
fn get_runtime_controls(runtime: tauri::State<RuntimeState>) -> RuntimeControls {
    runtime_controls(&SyncEngine::current(), &runtime)
}

#[tauri::command]
fn set_allow_filesystem_changes(
    allow: bool,
    runtime: tauri::State<RuntimeState>,
) -> Result<RuntimeControls, String> {
    let engine = SyncEngine::current();
    set_allow_filesystem_changes_inner(allow, &runtime, &engine)
}

#[tauri::command]
fn list_audit_events(
    limit: Option<usize>,
    status: Option<String>,
    action: Option<String>,
) -> Result<Vec<AuditEvent>, String> {
    let parsed_status = parse_audit_status(status.as_deref())?;
    let events = SyncEngine::current().list_audit_events(limit, parsed_status, action.as_deref());
    Ok(events)
}

#[tauri::command]
fn get_state() -> SyncState {
    SyncEngine::current().load_state()
}

#[tauri::command]
fn get_starred_skill_ids() -> Vec<String> {
    SyncEngine::current().starred_skill_ids()
}

#[tauri::command]
fn set_skill_starred(skill_id: String, starred: bool) -> Result<Vec<String>, String> {
    let engine = SyncEngine::current();
    let state = engine.load_state();
    if !state.skills.iter().any(|skill| skill.id == skill_id) {
        return Err(format!("skill id not found: {skill_id}"));
    }

    engine
        .set_skill_starred(&skill_id, starred)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_skills(scope: Option<String>) -> Result<Vec<SkillRecord>, String> {
    let engine = SyncEngine::current();
    let scope_filter = scope
        .as_deref()
        .map(|value| {
            value
                .parse::<ScopeFilter>()
                .map_err(|_| format!("unsupported scope: {value}"))
        })
        .transpose()?
        .unwrap_or(ScopeFilter::All);
    Ok(engine.list_skills(scope_filter))
}

#[tauri::command]
fn list_subagents(scope: Option<String>) -> Result<Vec<SubagentRecord>, String> {
    let engine = SyncEngine::current();
    let scope_filter = scope
        .as_deref()
        .map(|value| {
            value
                .parse::<ScopeFilter>()
                .map_err(|_| format!("unsupported scope: {value}"))
        })
        .transpose()?
        .unwrap_or(ScopeFilter::All);
    Ok(engine.list_subagents(scope_filter))
}

#[tauri::command]
fn run_dotagents_sync(
    scope: Option<String>,
    runtime: tauri::State<RuntimeState>,
) -> Result<(), String> {
    let engine = SyncEngine::current();
    ensure_write_allowed(&engine, "run_dotagents_sync")?;
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    let _guard = runtime
        .sync_lock
        .lock()
        .map_err(|_| String::from("internal lock error"))?;
    engine
        .run_dotagents_sync(parsed_scope)
        .map_err(|error| error.to_string())?;
    engine
        .run_dotagents_install_frozen(parsed_scope)
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn list_dotagents_skills(scope: Option<String>) -> Result<Vec<SkillRecord>, String> {
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    SyncEngine::current()
        .list_dotagents_skills(parsed_scope)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_dotagents_mcp(scope: Option<String>) -> Result<Vec<McpServerRecord>, String> {
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    SyncEngine::current()
        .list_dotagents_mcp(parsed_scope)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn dotagents_skills_install(
    scope: Option<String>,
    runtime: tauri::State<RuntimeState>,
) -> Result<(), String> {
    let engine = SyncEngine::current();
    ensure_write_allowed(&engine, "dotagents_skills_install")?;
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    let _guard = runtime
        .sync_lock
        .lock()
        .map_err(|_| String::from("internal lock error"))?;
    engine
        .run_dotagents_install_frozen(parsed_scope)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn dotagents_skills_add(
    package: String,
    scope: Option<String>,
    runtime: tauri::State<RuntimeState>,
) -> Result<(), String> {
    let engine = SyncEngine::current();
    ensure_write_allowed(&engine, "dotagents_skills_add")?;
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    let _guard = runtime
        .sync_lock
        .lock()
        .map_err(|_| String::from("internal lock error"))?;
    engine
        .run_dotagents_command(parsed_scope, &["add", package.as_str()])
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn dotagents_skills_remove(
    package: String,
    scope: Option<String>,
    runtime: tauri::State<RuntimeState>,
) -> Result<(), String> {
    let engine = SyncEngine::current();
    ensure_write_allowed(&engine, "dotagents_skills_remove")?;
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    let _guard = runtime
        .sync_lock
        .lock()
        .map_err(|_| String::from("internal lock error"))?;
    engine
        .run_dotagents_command(parsed_scope, &["remove", package.as_str()])
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn dotagents_skills_update(
    package: Option<String>,
    scope: Option<String>,
    runtime: tauri::State<RuntimeState>,
) -> Result<(), String> {
    let engine = SyncEngine::current();
    ensure_write_allowed(&engine, "dotagents_skills_update")?;
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    let _guard = runtime
        .sync_lock
        .lock()
        .map_err(|_| String::from("internal lock error"))?;

    let mut command = vec![String::from("update")];
    if let Some(pkg) = package {
        command.push(pkg);
    }
    let refs = command.iter().map(String::as_str).collect::<Vec<_>>();
    engine
        .run_dotagents_command(parsed_scope, &refs)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn dotagents_mcp_add(
    args: Vec<String>,
    scope: Option<String>,
    runtime: tauri::State<RuntimeState>,
) -> Result<(), String> {
    let engine = SyncEngine::current();
    ensure_write_allowed(&engine, "dotagents_mcp_add")?;
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    let _guard = runtime
        .sync_lock
        .lock()
        .map_err(|_| String::from("internal lock error"))?;
    let mut command = vec![String::from("mcp"), String::from("add")];
    command.extend(args);
    let refs = command.iter().map(String::as_str).collect::<Vec<_>>();
    engine
        .run_dotagents_command(parsed_scope, &refs)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn dotagents_mcp_remove(
    args: Vec<String>,
    scope: Option<String>,
    runtime: tauri::State<RuntimeState>,
) -> Result<(), String> {
    let engine = SyncEngine::current();
    ensure_write_allowed(&engine, "dotagents_mcp_remove")?;
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    let _guard = runtime
        .sync_lock
        .lock()
        .map_err(|_| String::from("internal lock error"))?;
    let mut command = vec![String::from("mcp"), String::from("remove")];
    command.extend(args);
    let refs = command.iter().map(String::as_str).collect::<Vec<_>>();
    engine
        .run_dotagents_command(parsed_scope, &refs)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn migrate_dotagents(
    scope: Option<String>,
    runtime: tauri::State<RuntimeState>,
) -> Result<(), String> {
    let engine = SyncEngine::current();
    ensure_write_allowed(&engine, "migrate_dotagents")?;
    let parsed_scope = parse_dotagents_scope(scope.as_deref())?;
    let _guard = runtime
        .sync_lock
        .lock()
        .map_err(|_| String::from("internal lock error"))?;
    engine
        .migrate_to_dotagents(parsed_scope)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_mcp_servers() -> Vec<McpServerRecord> {
    SyncEngine::current().list_mcp_servers()
}

#[tauri::command]
fn set_mcp_server_enabled(
    server_key: String,
    agent: String,
    enabled: bool,
    scope: Option<String>,
    workspace: Option<String>,
    runtime: tauri::State<RuntimeState>,
) -> Result<SyncState, String> {
    let engine = SyncEngine::current();
    ensure_write_allowed(&engine, "set_mcp_server_enabled")?;
    let parsed = agent
        .parse::<McpAgent>()
        .map_err(|error| error.to_string())?;
    let _guard = runtime
        .sync_lock
        .lock()
        .map_err(|_| String::from("internal lock error"))?;
    engine
        .set_mcp_server_enabled(
            &server_key,
            parsed,
            enabled,
            scope.as_deref(),
            workspace.as_deref(),
        )
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_skill(
    skill_key: String,
    confirmed: bool,
    runtime: tauri::State<RuntimeState>,
) -> Result<SyncState, String> {
    let engine = SyncEngine::current();
    ensure_write_allowed(&engine, "delete_skill")?;
    let _guard = runtime
        .sync_lock
        .lock()
        .map_err(|_| String::from("internal lock error"))?;
    let skill = find_skill(&engine, &skill_key, None)?;
    engine
        .delete(&skill, confirmed)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn archive_skill(
    skill_key: String,
    confirmed: bool,
    runtime: tauri::State<RuntimeState>,
) -> Result<SyncState, String> {
    let engine = SyncEngine::current();
    ensure_write_allowed(&engine, "archive_skill")?;
    let _guard = runtime
        .sync_lock
        .lock()
        .map_err(|_| String::from("internal lock error"))?;
    let skill = find_skill(&engine, &skill_key, Some(SkillLifecycleStatus::Active))?;
    engine
        .archive(&skill, confirmed)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn restore_skill(
    skill_key: String,
    confirmed: bool,
    runtime: tauri::State<RuntimeState>,
) -> Result<SyncState, String> {
    let engine = SyncEngine::current();
    ensure_write_allowed(&engine, "restore_skill")?;
    let _guard = runtime
        .sync_lock
        .lock()
        .map_err(|_| String::from("internal lock error"))?;
    let skill = find_skill(&engine, &skill_key, Some(SkillLifecycleStatus::Archived))?;
    engine
        .restore(&skill, confirmed)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn make_global(
    skill_key: String,
    confirmed: bool,
    runtime: tauri::State<RuntimeState>,
) -> Result<SyncState, String> {
    let engine = SyncEngine::current();
    ensure_write_allowed(&engine, "make_global")?;
    let _guard = runtime
        .sync_lock
        .lock()
        .map_err(|_| String::from("internal lock error"))?;
    let skill = find_skill(&engine, &skill_key, Some(SkillLifecycleStatus::Active))?;
    engine
        .make_global(&skill, confirmed)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn rename_skill(
    skill_key: String,
    new_title: String,
    runtime: tauri::State<RuntimeState>,
) -> Result<SyncState, String> {
    let engine = SyncEngine::current();
    ensure_write_allowed(&engine, "rename_skill")?;
    let _guard = runtime
        .sync_lock
        .lock()
        .map_err(|_| String::from("internal lock error"))?;
    let skill = find_skill(&engine, &skill_key, Some(SkillLifecycleStatus::Active))?;
    engine
        .rename(&skill, &new_title)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_skill_details(skill_key: String) -> Result<SkillDetails, String> {
    let engine = SyncEngine::current();
    let skill = find_skill(&engine, &skill_key, None)?;
    let main_file = resolve_main_skill_file(&skill);
    let skill_root = resolve_skill_root_dir(&skill, &main_file);
    let main_file_exists = main_file.exists();
    let (main_file_body_preview, main_file_body_preview_truncated) =
        read_preview(&main_file, MAX_MAIN_FILE_PREVIEW_CHARS);
    let (skill_dir_tree_preview, skill_dir_tree_preview_truncated) =
        read_skill_dir_tree(&skill_root, MAX_TREE_ENTRIES);
    let last_modified_unix_seconds = fs::metadata(&main_file)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|ts| ts.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs());

    Ok(SkillDetails {
        skill,
        main_file_path: main_file.display().to_string(),
        main_file_exists,
        main_file_body_preview,
        main_file_body_preview_truncated,
        skill_dir_tree_preview,
        skill_dir_tree_preview_truncated,
        last_modified_unix_seconds,
    })
}

#[tauri::command]
fn get_subagent_details(subagent_id: String) -> Result<SubagentDetails, String> {
    let engine = SyncEngine::current();
    let subagent = find_subagent(&engine, &subagent_id)?;
    let main_file = PathBuf::from(&subagent.canonical_source_path);
    let canonical_source = fs::canonicalize(&main_file).ok();
    let subagent_root = main_file
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(&subagent.canonical_source_path));
    let main_file_exists = main_file.exists();
    let (main_file_body_preview, main_file_body_preview_truncated) =
        read_preview(&main_file, MAX_MAIN_FILE_PREVIEW_CHARS);
    let (subagent_dir_tree_preview, subagent_dir_tree_preview_truncated) =
        read_skill_dir_tree(&subagent_root, MAX_TREE_ENTRIES);
    let last_modified_unix_seconds = fs::metadata(&main_file)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|ts| ts.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs());
    let target_statuses = collect_subagent_target_statuses(
        &subagent.target_paths,
        main_file.parent(),
        canonical_source.as_deref(),
    );

    Ok(SubagentDetails {
        subagent,
        main_file_path: main_file.display().to_string(),
        main_file_exists,
        main_file_body_preview,
        main_file_body_preview_truncated,
        subagent_dir_tree_preview,
        subagent_dir_tree_preview_truncated,
        last_modified_unix_seconds,
        target_statuses,
    })
}

#[tauri::command]
fn open_skill_path(skill_key: String, target: Option<String>) -> Result<(), String> {
    let engine = SyncEngine::current();
    let skill = find_skill(&engine, &skill_key, None)?;
    let selected_target = target.unwrap_or_else(|| String::from("folder"));
    let path = match selected_target.as_str() {
        "folder" => PathBuf::from(&skill.canonical_source_path),
        "file" => resolve_main_skill_file(&skill),
        other => {
            return Err(format!(
                "unsupported target: {other} (allowed: folder|file)"
            ));
        }
    };
    open_path(&path)
}

#[tauri::command]
fn open_subagent_path(subagent_id: String, target: Option<String>) -> Result<(), String> {
    let engine = SyncEngine::current();
    let subagent = find_subagent(&engine, &subagent_id)?;
    let selected_target = target.unwrap_or_else(|| String::from("folder"));
    let path = match selected_target.as_str() {
        "folder" => PathBuf::from(&subagent.canonical_source_path)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(&subagent.canonical_source_path)),
        "file" => PathBuf::from(&subagent.canonical_source_path),
        other => {
            return Err(format!(
                "unsupported target: {other} (allowed: folder|file)"
            ));
        }
    };
    open_path(&path)
}

#[tauri::command]
fn get_platform_context() -> PlatformContext {
    let linux_desktop_raw = if normalize_os_name(std::env::consts::OS) == "linux" {
        std::env::var("XDG_CURRENT_DESKTOP").ok()
    } else {
        None
    };
    build_platform_context(std::env::consts::OS, linux_desktop_raw.as_deref())
}

fn normalize_os_name(raw_os: &str) -> &'static str {
    match raw_os {
        "macos" => "macos",
        "windows" => "windows",
        "linux" => "linux",
        _ => "unknown",
    }
}

fn normalize_linux_desktop(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn build_platform_context(raw_os: &str, linux_desktop_raw: Option<&str>) -> PlatformContext {
    let os = normalize_os_name(raw_os);
    let linux_desktop = if os == "linux" {
        normalize_linux_desktop(linux_desktop_raw)
    } else {
        None
    };
    PlatformContext {
        os: os.to_owned(),
        linux_desktop,
    }
}

fn find_skill(
    engine: &SyncEngine,
    skill_key: &str,
    status: Option<SkillLifecycleStatus>,
) -> Result<SkillRecord, String> {
    engine
        .find_skill(&SkillLocator {
            skill_key: skill_key.to_owned(),
            status,
        })
        .ok_or_else(|| format!("skill not found: {skill_key}"))
}

fn find_subagent(engine: &SyncEngine, subagent_id: &str) -> Result<SubagentRecord, String> {
    engine
        .find_subagent_by_id(subagent_id)
        .ok_or_else(|| format!("subagent not found: {subagent_id}"))
}

fn resolve_main_skill_file(skill: &SkillRecord) -> PathBuf {
    let source = PathBuf::from(&skill.canonical_source_path);
    if skill.package_type == "dir" {
        source.join("SKILL.md")
    } else {
        source
    }
}

fn resolve_skill_root_dir(skill: &SkillRecord, main_file: &Path) -> PathBuf {
    if skill.package_type == "dir" {
        return PathBuf::from(&skill.canonical_source_path);
    }
    main_file
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(&skill.canonical_source_path))
}

fn read_preview(path: &Path, max_chars: usize) -> (Option<String>, bool) {
    let Ok(contents) = fs::read_to_string(path) else {
        return (None, false);
    };

    let total_chars = contents.chars().count();
    if total_chars <= max_chars {
        return (Some(contents), false);
    }

    let preview = contents.chars().take(max_chars).collect::<String>();
    (Some(preview), true)
}

fn read_skill_dir_tree(root: &Path, max_entries: usize) -> (Option<String>, bool) {
    if max_entries == 0 {
        return (None, false);
    }

    let Ok(metadata) = fs::symlink_metadata(root) else {
        return (None, false);
    };
    if !metadata.file_type().is_dir() {
        return (None, false);
    }

    let mut lines = Vec::new();
    let root_label = root
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| root.display().to_string());
    lines.push(format!("{root_label}/"));

    let mut emitted_entries: usize = 0;
    let mut truncated = false;
    render_tree_entries(
        root,
        "",
        &mut lines,
        max_entries,
        &mut emitted_entries,
        &mut truncated,
    );

    (Some(lines.join("\n")), truncated)
}

fn collect_subagent_target_statuses(
    target_paths: &[String],
    canonical_parent: Option<&Path>,
    canonical_source: Option<&Path>,
) -> Vec<SubagentTargetStatus> {
    target_paths
        .iter()
        .map(|path| build_subagent_target_status(path, canonical_parent, canonical_source))
        .collect()
}

fn build_subagent_target_status(
    path: &str,
    canonical_parent: Option<&Path>,
    canonical_source: Option<&Path>,
) -> SubagentTargetStatus {
    let target_path = PathBuf::from(path);
    let Ok(metadata) = fs::symlink_metadata(&target_path) else {
        return SubagentTargetStatus {
            path: path.to_owned(),
            exists: false,
            is_symlink: false,
            symlink_target: None,
            points_to_canonical: false,
            kind: SubagentTargetKind::Missing,
        };
    };

    let file_type = metadata.file_type();
    let is_symlink = file_type.is_symlink();

    if is_symlink {
        let raw_link = fs::read_link(&target_path).ok();
        let resolved_link = raw_link.as_ref().map(|link| {
            if link.is_absolute() {
                link.clone()
            } else {
                target_path
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_default()
                    .join(link)
            }
        });
        let points_to_canonical = resolved_link
            .as_deref()
            .and_then(|resolved| canonicalize_with_base(resolved, canonical_parent))
            .zip(canonical_source)
            .map(|(resolved, canonical)| resolved == canonical)
            .unwrap_or(false);

        return SubagentTargetStatus {
            path: path.to_owned(),
            exists: true,
            is_symlink: true,
            symlink_target: raw_link.map(|link| link.display().to_string()),
            points_to_canonical,
            kind: SubagentTargetKind::Symlink,
        };
    }

    if file_type.is_file() {
        return SubagentTargetStatus {
            path: path.to_owned(),
            exists: true,
            is_symlink: false,
            symlink_target: None,
            points_to_canonical: false,
            kind: SubagentTargetKind::RegularFile,
        };
    }

    SubagentTargetStatus {
        path: path.to_owned(),
        exists: true,
        is_symlink: false,
        symlink_target: None,
        points_to_canonical: false,
        kind: SubagentTargetKind::Other,
    }
}

fn canonicalize_with_base(path: &Path, base_dir: Option<&Path>) -> Option<PathBuf> {
    fs::canonicalize(path).ok().or_else(|| {
        base_dir
            .map(|base| base.join(path))
            .and_then(|joined| fs::canonicalize(joined).ok())
    })
}

fn render_tree_entries(
    dir: &Path,
    prefix: &str,
    lines: &mut Vec<String>,
    max_entries: usize,
    emitted_entries: &mut usize,
    truncated: &mut bool,
) {
    if *truncated {
        return;
    }

    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    let mut children: Vec<(String, bool, PathBuf)> = entries
        .filter_map(Result::ok)
        .map(|entry| {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = fs::symlink_metadata(&path)
                .map(|meta| meta.file_type().is_dir())
                .unwrap_or(false);
            (name, is_dir, path)
        })
        .collect();

    children.sort_by(|lhs, rhs| match (lhs.1, rhs.1) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => lhs
            .0
            .to_lowercase()
            .cmp(&rhs.0.to_lowercase())
            .then_with(|| lhs.0.cmp(&rhs.0)),
    });

    let child_count = children.len();
    for (index, (name, is_dir, path)) in children.into_iter().enumerate() {
        if *emitted_entries >= max_entries {
            *truncated = true;
            return;
        }

        let is_last = index + 1 == child_count;
        let branch = if is_last { "`-- " } else { "|-- " };
        let label = if is_dir { format!("{name}/") } else { name };
        lines.push(format!("{prefix}{branch}{label}"));
        *emitted_entries += 1;

        if is_dir {
            let next_prefix = if is_last {
                format!("{prefix}    ")
            } else {
                format!("{prefix}|   ")
            };
            render_tree_entries(
                &path,
                &next_prefix,
                lines,
                max_entries,
                emitted_entries,
                truncated,
            );
            if *truncated {
                return;
            }
        }
    }
}

fn open_path(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("path does not exist: {}", path.display()));
    }

    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut cmd = Command::new("open");
        cmd.arg(path);
        cmd
    };

    #[cfg(target_os = "linux")]
    let mut cmd = {
        let mut cmd = Command::new("xdg-open");
        cmd.arg(path);
        cmd
    };

    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg("start").arg("").arg(path);
        cmd
    };

    let status = cmd
        .status()
        .map_err(|error| format!("failed to launch opener for {}: {}", path.display(), error))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "opener exited with status {} for {}",
            status,
            path.display()
        ))
    }
}

fn main() {
    let runtime_state = RuntimeState::default();
    tauri::Builder::default()
        .manage(runtime_state)
        .setup(|app| {
            let runtime = app.state::<RuntimeState>();
            let engine = SyncEngine::current();
            if engine.allow_filesystem_changes() {
                if let Err(error) = enable_auto_watch_and_initial_sync(&runtime, &engine) {
                    eprintln!("failed to start auto watch on startup: {error}");
                    let _ = engine.set_allow_filesystem_changes(false);
                    stop_auto_watch(&runtime);
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            run_sync,
            get_runtime_controls,
            set_allow_filesystem_changes,
            list_audit_events,
            get_state,
            get_starred_skill_ids,
            set_skill_starred,
            list_skills,
            list_subagents,
            run_dotagents_sync,
            list_dotagents_skills,
            list_dotagents_mcp,
            dotagents_skills_install,
            dotagents_skills_add,
            dotagents_skills_remove,
            dotagents_skills_update,
            dotagents_mcp_add,
            dotagents_mcp_remove,
            migrate_dotagents,
            get_mcp_servers,
            set_mcp_server_enabled,
            delete_skill,
            archive_skill,
            restore_skill,
            make_global,
            rename_skill,
            get_skill_details,
            get_subagent_details,
            open_skill_path,
            open_subagent_path,
            get_platform_context,
        ])
        .run(tauri::generate_context!())
        .expect("error while running skillssync desktop app");
}

#[cfg(test)]
mod tests {
    use super::{
        build_platform_context, build_subagent_target_status, enable_auto_watch_and_initial_sync,
        ensure_write_allowed, normalize_os_name, read_skill_dir_tree,
        set_allow_filesystem_changes_inner, set_allow_filesystem_changes_inner_with,
        stop_auto_watch, RuntimeState, SubagentTargetKind,
    };
    use skillssync_core::{
        AuditEventStatus, SyncEngine, SyncEngineEnvironment, SyncPaths, SyncPreferencesStore,
        SyncStateStore,
    };
    use std::fs;
    use std::path::Path;
    use std::sync::mpsc;
    use std::time::Duration;
    use tempfile::{tempdir, TempDir};

    fn engine_in_temp(temp: &TempDir) -> SyncEngine {
        let home = temp.path().join("home");
        let runtime = temp.path().join("runtime");
        let app_runtime = temp.path().join("app-runtime");
        fs::create_dir_all(&home).expect("home");
        fs::create_dir_all(&runtime).expect("runtime");
        fs::create_dir_all(&app_runtime).expect("app runtime");

        let env = SyncEngineEnvironment {
            home_directory: home.clone(),
            dev_root: home.join("Dev"),
            worktrees_root: home.join(".codex").join("worktrees"),
            runtime_directory: runtime,
        };

        let paths = SyncPaths::from_runtime(app_runtime);
        let store = SyncStateStore::new(paths.clone());
        let prefs = SyncPreferencesStore::new(paths);
        SyncEngine::new(env, store, prefs)
    }

    #[test]
    fn read_skill_dir_tree_returns_stable_ascii_structure() {
        let tempdir = tempdir().expect("create temp dir");
        let root = tempdir.path().join("alpha-skill");
        fs::create_dir_all(root.join("references")).expect("create nested dir");
        fs::write(root.join("SKILL.md"), "# Skill").expect("write SKILL.md");
        fs::write(root.join("README.md"), "readme").expect("write README.md");
        fs::write(root.join("references").join("notes.md"), "notes").expect("write nested file");

        let (tree, truncated) = read_skill_dir_tree(Path::new(&root), 1000);

        assert!(!truncated);
        assert_eq!(
            tree,
            Some(String::from(
                "alpha-skill/\n|-- references/\n|   `-- notes.md\n|-- README.md\n`-- SKILL.md"
            ))
        );
    }

    #[test]
    fn read_skill_dir_tree_marks_truncation_when_limit_reached() {
        let tempdir = tempdir().expect("create temp dir");
        let root = tempdir.path().join("beta-skill");
        fs::create_dir_all(&root).expect("create root");
        fs::write(root.join("a.md"), "a").expect("write a");
        fs::write(root.join("b.md"), "b").expect("write b");
        fs::write(root.join("c.md"), "c").expect("write c");

        let (tree, truncated) = read_skill_dir_tree(Path::new(&root), 2);

        assert!(truncated);
        assert_eq!(tree, Some(String::from("beta-skill/\n|-- a.md\n|-- b.md")));
    }

    #[test]
    fn normalize_os_name_maps_supported_values() {
        assert_eq!(normalize_os_name("macos"), "macos");
        assert_eq!(normalize_os_name("windows"), "windows");
        assert_eq!(normalize_os_name("linux"), "linux");
        assert_eq!(normalize_os_name("freebsd"), "unknown");
    }

    #[test]
    fn build_platform_context_sets_linux_desktop_only_for_linux() {
        let linux = build_platform_context("linux", Some("GNOME"));
        assert_eq!(linux.os, "linux");
        assert_eq!(linux.linux_desktop, Some(String::from("GNOME")));

        let mac = build_platform_context("macos", Some("GNOME"));
        assert_eq!(mac.os, "macos");
        assert_eq!(mac.linux_desktop, None);
    }

    #[test]
    fn build_subagent_target_status_marks_missing_path() {
        let status = build_subagent_target_status("/tmp/does-not-exist/subagent.md", None, None);
        assert_eq!(status.kind, SubagentTargetKind::Missing);
        assert!(!status.exists);
        assert!(!status.is_symlink);
        assert_eq!(status.symlink_target, None);
        assert!(!status.points_to_canonical);
    }

    #[test]
    fn build_subagent_target_status_marks_regular_file() {
        let dir = tempdir().expect("create tempdir");
        let target_file = dir.path().join("subagent.md");
        fs::write(&target_file, "hello").expect("write file");

        let status = build_subagent_target_status(&target_file.display().to_string(), None, None);
        assert_eq!(status.kind, SubagentTargetKind::RegularFile);
        assert!(status.exists);
        assert!(!status.is_symlink);
        assert_eq!(status.symlink_target, None);
        assert!(!status.points_to_canonical);
    }

    #[cfg(unix)]
    #[test]
    fn build_subagent_target_status_marks_symlink_and_canonical_match() {
        use std::os::unix::fs as unix_fs;

        let dir = tempdir().expect("create tempdir");
        let canonical = dir.path().join("canonical.md");
        fs::write(&canonical, "hello").expect("write canonical");

        let links_dir = dir.path().join("links");
        fs::create_dir_all(&links_dir).expect("create links dir");
        let link_path = links_dir.join("subagent.md");
        unix_fs::symlink("../canonical.md", &link_path).expect("create symlink");

        let canonical_path = fs::canonicalize(&canonical).expect("canonicalize");
        let status = build_subagent_target_status(
            &link_path.display().to_string(),
            None,
            Some(canonical_path.as_path()),
        );

        assert_eq!(status.kind, SubagentTargetKind::Symlink);
        assert!(status.exists);
        assert!(status.is_symlink);
        assert_eq!(status.symlink_target, Some(String::from("../canonical.md")));
        assert!(status.points_to_canonical);
    }

    #[cfg(unix)]
    #[test]
    fn build_subagent_target_status_handles_broken_symlink() {
        use std::os::unix::fs as unix_fs;

        let dir = tempdir().expect("create tempdir");
        let link_path = dir.path().join("broken.md");
        unix_fs::symlink("missing.md", &link_path).expect("create broken symlink");

        let status = build_subagent_target_status(&link_path.display().to_string(), None, None);
        assert_eq!(status.kind, SubagentTargetKind::Symlink);
        assert!(status.exists);
        assert!(status.is_symlink);
        assert_eq!(status.symlink_target, Some(String::from("missing.md")));
        assert!(!status.points_to_canonical);
    }

    #[test]
    fn ensure_write_allowed_does_not_append_blocked_audit() {
        let temp = tempdir().expect("create tempdir");
        let engine = engine_in_temp(&temp);
        engine
            .set_allow_filesystem_changes(false)
            .expect("disable filesystem changes");

        let before =
            engine.list_audit_events(Some(10), Some(AuditEventStatus::Blocked), Some("run_sync"));
        assert!(before.is_empty());

        let result = ensure_write_allowed(&engine, "run_sync");
        assert!(result.is_err());

        let after =
            engine.list_audit_events(Some(10), Some(AuditEventStatus::Blocked), Some("run_sync"));
        assert!(after.is_empty());
    }

    #[test]
    fn enable_auto_watch_waits_for_initial_sync_before_returning() {
        let temp = tempdir().expect("create tempdir");
        let engine = engine_in_temp(&temp);
        engine
            .set_allow_filesystem_changes(true)
            .expect("enable filesystem changes");
        let runtime = RuntimeState::default();

        let guard = runtime
            .sync_lock
            .lock()
            .expect("lock sync mutex to block initial sync");
        let (tx, rx) = mpsc::channel();
        let runtime_for_thread = runtime.clone();
        let engine_for_thread = engine.clone();
        let join = std::thread::spawn(move || {
            let result =
                enable_auto_watch_and_initial_sync(&runtime_for_thread, &engine_for_thread);
            tx.send(result).expect("send startup result");
        });

        assert!(
            rx.recv_timeout(Duration::from_secs(1)).is_err(),
            "startup should wait for initial sync lock"
        );

        drop(guard);
        let result = rx
            .recv_timeout(Duration::from_secs(30))
            .expect("startup should finish once lock is released");
        assert!(result.is_ok());

        stop_auto_watch(&runtime);
        join.join().expect("join startup thread");
    }

    #[test]
    fn set_allow_filesystem_changes_reports_startup_errors_and_reverts_writes() {
        let temp = tempdir().expect("create tempdir");
        let engine = engine_in_temp(&temp);
        let runtime = RuntimeState::default();

        let result = set_allow_filesystem_changes_inner_with(true, &runtime, &engine, |_r, _e| {
            Err(String::from("watch startup failed"))
        });

        assert!(matches!(result, Err(error) if error == "watch startup failed"));
        assert!(!engine.allow_filesystem_changes());
        assert!(ensure_write_allowed(&engine, "delete_skill").is_err());
    }

    #[test]
    fn set_allow_filesystem_changes_reverts_write_mode_when_initial_sync_fails() {
        let temp = tempdir().expect("create tempdir");
        let engine = engine_in_temp(&temp);
        let runtime = RuntimeState::default();
        let home = engine.environment().home_directory.clone();

        let claude_skill = home.join(".claude").join("skills").join("duplicate");
        fs::create_dir_all(&claude_skill).expect("create claude skill dir");
        fs::write(claude_skill.join("SKILL.md"), "# A").expect("write claude skill");

        let agents_skill = home.join(".agents").join("skills").join("duplicate");
        fs::create_dir_all(&agents_skill).expect("create agents skill dir");
        fs::write(agents_skill.join("SKILL.md"), "# B").expect("write agents skill");

        let result = set_allow_filesystem_changes_inner(true, &runtime, &engine);

        assert!(matches!(result, Err(error) if error.contains("Detected 1 conflict")));
        assert!(!engine.allow_filesystem_changes());
        assert!(ensure_write_allowed(&engine, "delete_skill").is_err());
    }
}
