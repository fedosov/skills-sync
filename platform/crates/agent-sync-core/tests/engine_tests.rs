use agent_sync_core::{
    AuditEventStatus, CatalogMutationAction, CatalogMutationTarget, DotagentsScope, McpAgent,
    ScopeFilter, SkillLifecycleStatus, SkillLocator, SyncEngine, SyncEngineEnvironment, SyncPaths,
    SyncPreferencesStore, SyncStateStore, SyncTrigger,
};
use serde_json::Value as JsonValue;
use std::ffi::OsString;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use tempfile::TempDir;

fn write_skill(root: &Path, key: &str, body: &str) {
    let skill_path = root.join(key).join("SKILL.md");
    fs::create_dir_all(skill_path.parent().expect("parent")).expect("create parent");
    fs::write(skill_path, body).expect("write skill");
}

fn write_subagent(root: &Path, key: &str, body: &str) {
    let subagent_path = root.join(format!("{key}.md"));
    fs::create_dir_all(subagent_path.parent().expect("parent")).expect("create parent");
    fs::write(subagent_path, body).expect("write subagent");
}

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

fn app_settings_path(temp: &TempDir) -> std::path::PathBuf {
    temp.path().join("app-runtime").join("app-settings.json")
}

fn write_text(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, body).expect("write file");
}

fn count_occurrences(body: &str, needle: &str) -> usize {
    body.match_indices(needle).count()
}

fn dotagents_env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct EnvVarRestore {
    key: &'static str,
    previous: Option<OsString>,
}

impl Drop for EnvVarRestore {
    fn drop(&mut self) {
        if let Some(value) = self.previous.take() {
            std::env::set_var(self.key, value);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

fn set_env_var_with_restore(key: &'static str, value: &Path) -> EnvVarRestore {
    let previous = std::env::var_os(key);
    std::env::set_var(key, value);
    EnvVarRestore { key, previous }
}

fn find_skill(
    engine: &SyncEngine,
    skill_key: &str,
    status: Option<SkillLifecycleStatus>,
) -> agent_sync_core::SkillRecord {
    engine
        .find_skill(&SkillLocator {
            skill_key: String::from(skill_key),
            status,
        })
        .expect("skill exists")
}

fn find_mcp<'a>(
    state: &'a agent_sync_core::SyncState,
    server_key: &str,
    scope: &str,
    workspace: Option<&str>,
) -> &'a agent_sync_core::McpServerRecord {
    state
        .mcp_servers
        .iter()
        .find(|item| {
            item.server_key == server_key
                && item.scope == scope
                && item.workspace.as_deref() == workspace
        })
        .expect("mcp record exists")
}

fn find_subagent<'a>(
    state: &'a agent_sync_core::SyncState,
    subagent_key: &str,
    status: Option<SkillLifecycleStatus>,
) -> &'a agent_sync_core::SubagentRecord {
    state
        .subagents
        .iter()
        .find(|item| {
            item.subagent_key == subagent_key
                && status.map(|value| item.status == value).unwrap_or(true)
        })
        .expect("subagent exists")
}

#[test]
fn run_sync_builds_and_persists_state() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    write_skill(
        &engine
            .environment()
            .home_directory
            .join(".claude")
            .join("skills"),
        "alpha",
        "# A",
    );
    write_skill(
        &engine
            .environment()
            .home_directory
            .join(".agents")
            .join("skills"),
        "beta",
        "# B",
    );

    let workspace = engine
        .environment()
        .home_directory
        .join("Dev")
        .join("workspace-a");
    write_skill(
        &workspace.join(".claude").join("skills"),
        "project-1",
        "# P",
    );

    let state = engine.run_sync(SyncTrigger::Manual).expect("sync state");
    assert_eq!(state.summary.global_count, 2);
    assert_eq!(state.summary.project_count, 1);
    assert_eq!(state.summary.conflict_count, 0);
    assert_eq!(engine.load_state().skills.len(), 3);
}

#[test]
fn run_sync_records_success_audit_event() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    write_skill(
        &engine
            .environment()
            .home_directory
            .join(".claude")
            .join("skills"),
        "alpha",
        "# A",
    );

    let _ = engine.run_sync(SyncTrigger::Manual).expect("sync state");
    let events =
        engine.list_audit_events(Some(20), Some(AuditEventStatus::Success), Some("run_sync"));

    assert!(!events.is_empty());
    let event = &events[0];
    assert_eq!(event.action, "run_sync");
    assert_eq!(event.trigger.as_deref(), Some("manual"));
}

#[test]
fn run_sync_skips_success_audit_when_no_managed_changes() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    write_skill(
        &engine
            .environment()
            .home_directory
            .join(".claude")
            .join("skills"),
        "alpha",
        "# A",
    );

    let _ = engine.run_sync(SyncTrigger::Manual).expect("first sync");
    let first_count = engine
        .list_audit_events(Some(100), Some(AuditEventStatus::Success), Some("run_sync"))
        .len();
    assert!(first_count >= 1);

    let _ = engine.run_sync(SyncTrigger::Manual).expect("second sync");
    let second_count = engine
        .list_audit_events(Some(100), Some(AuditEventStatus::Success), Some("run_sync"))
        .len();
    assert_eq!(second_count, first_count);
}

#[test]
fn run_sync_records_success_audit_when_recovering_from_failed_sync() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    let claude_skills = engine
        .environment()
        .home_directory
        .join(".claude")
        .join("skills");
    write_skill(&claude_skills, "alpha", "# A");

    let _ = engine.run_sync(SyncTrigger::Manual).expect("initial sync");
    let first_success_count = engine
        .list_audit_events(Some(100), Some(AuditEventStatus::Success), Some("run_sync"))
        .len();

    let agents_skills = engine
        .environment()
        .home_directory
        .join(".agents")
        .join("skills");
    let conflicting_agents_skill = agents_skills.join("alpha");
    let metadata =
        fs::symlink_metadata(&conflicting_agents_skill).expect("managed target metadata");
    if metadata.file_type().is_symlink() {
        fs::remove_file(&conflicting_agents_skill).expect("remove managed symlink");
    } else {
        fs::remove_dir_all(&conflicting_agents_skill).expect("remove managed directory");
    }
    write_skill(&agents_skills, "alpha", "# B");
    let conflict = engine
        .run_sync(SyncTrigger::Manual)
        .expect_err("sync should fail with conflict");
    assert!(conflict.to_string().contains("Detected 1 conflict"));

    let failed_count = engine
        .list_audit_events(Some(100), Some(AuditEventStatus::Failed), Some("run_sync"))
        .len();
    assert!(failed_count >= 1);

    fs::remove_dir_all(&conflicting_agents_skill).expect("remove conflicting skill");

    let _ = engine.run_sync(SyncTrigger::Manual).expect("recovery sync");
    let recovered_success_count = engine
        .list_audit_events(Some(100), Some(AuditEventStatus::Success), Some("run_sync"))
        .len();
    assert_eq!(recovered_success_count, first_success_count + 1);
}

#[test]
fn run_sync_records_success_audit_when_mcp_definition_changes_with_same_targets() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);
    let claude_user = engine.environment().home_directory.join(".claude.json");
    let central_catalog = engine
        .environment()
        .home_directory
        .join(".config")
        .join("ai-agents")
        .join("config.toml");

    write_text(
        &claude_user,
        r#"{
  "mcpServers": {
    "exa": {
      "type": "http",
      "url": "https://a.exa.ai/mcp"
    }
  }
}
"#,
    );

    let first_state = engine.run_sync(SyncTrigger::Manual).expect("first sync");
    let first_count = engine
        .list_audit_events(Some(100), Some(AuditEventStatus::Success), Some("run_sync"))
        .len();

    let catalog_raw = fs::read_to_string(&central_catalog).expect("read central catalog");
    let next_catalog_raw = catalog_raw.replace("https://a.exa.ai/mcp", "https://b.exa.ai/mcp");
    assert_ne!(next_catalog_raw, catalog_raw);
    write_text(&central_catalog, &next_catalog_raw);

    let second_state = engine.run_sync(SyncTrigger::Manual).expect("second sync");
    let second_count = engine
        .list_audit_events(Some(100), Some(AuditEventStatus::Success), Some("run_sync"))
        .len();

    let first_exa = find_mcp(&first_state, "exa", "global", None);
    let second_exa = find_mcp(&second_state, "exa", "global", None);
    assert_eq!(first_exa.targets, second_exa.targets);
    assert_ne!(first_exa.url, second_exa.url);
    assert_eq!(second_count, first_count + 1);
}

#[test]
fn run_sync_reports_conflict_when_hashes_differ() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    write_skill(
        &engine
            .environment()
            .home_directory
            .join(".claude")
            .join("skills"),
        "duplicate",
        "# A",
    );
    write_skill(
        &engine
            .environment()
            .home_directory
            .join(".agents")
            .join("skills"),
        "duplicate",
        "# B",
    );

    let error = engine.run_sync(SyncTrigger::Manual).expect_err("must fail");
    assert!(error.to_string().contains("Detected 1 conflict"));
    let persisted = engine.load_state();
    assert_eq!(
        persisted.sync.status,
        agent_sync_core::SyncHealthStatus::Failed
    );
    assert_eq!(persisted.summary.conflict_count, 1);
    assert_eq!(persisted.subagent_summary.conflict_count, 0);

    let failed_events =
        engine.list_audit_events(Some(20), Some(AuditEventStatus::Failed), Some("run_sync"));
    assert!(!failed_events.is_empty());
    assert_eq!(failed_events[0].action, "run_sync");
}

#[test]
fn rename_skill_updates_title_and_key() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    write_skill(
        &engine
            .environment()
            .home_directory
            .join(".claude")
            .join("skills"),
        "old-key",
        "---\ntitle: Old\n---\n\nBody",
    );

    let _ = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let skill = engine
        .find_skill(&SkillLocator {
            skill_key: String::from("old-key"),
            status: Some(SkillLifecycleStatus::Active),
        })
        .expect("skill exists");

    let state = engine.rename(&skill, "New Name").expect("rename");
    assert!(state.skills.iter().any(|item| item.skill_key == "new-name"));

    let skill_file = engine
        .environment()
        .home_directory
        .join(".claude")
        .join("skills")
        .join("new-name")
        .join("SKILL.md");
    let body = fs::read_to_string(skill_file).expect("read renamed file");
    assert!(body.contains("title: New Name"));
}

#[test]
fn list_skills_filters_archived() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    write_skill(
        &engine
            .environment()
            .home_directory
            .join(".claude")
            .join("skills"),
        "alpha",
        "# A",
    );
    let _ = engine.run_sync(SyncTrigger::Manual).expect("sync");

    let all = engine.list_skills(ScopeFilter::All);
    let archived = engine.list_skills(ScopeFilter::Archived);
    assert_eq!(all.len(), 1);
    assert_eq!(archived.len(), 0);
}

#[test]
fn archive_moves_skill_to_archives_and_marks_as_archived() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    write_skill(
        &engine
            .environment()
            .home_directory
            .join(".claude")
            .join("skills"),
        "alpha",
        "# A",
    );

    let _ = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let active = find_skill(&engine, "alpha", Some(SkillLifecycleStatus::Active));

    let state = engine.archive(&active, true).expect("archive");
    let archived = state
        .skills
        .iter()
        .find(|item| item.skill_key == "alpha" && item.status == SkillLifecycleStatus::Archived)
        .expect("archived skill entry");

    assert!(archived.archived_bundle_path.is_some());
    assert!(archived.canonical_source_path.contains("/archives/"));
    assert!(!engine
        .environment()
        .home_directory
        .join(".claude")
        .join("skills")
        .join("alpha")
        .exists());
}

#[test]
fn archive_requires_confirmation() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    write_skill(
        &engine
            .environment()
            .home_directory
            .join(".claude")
            .join("skills"),
        "alpha",
        "# A",
    );

    let _ = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let active = find_skill(&engine, "alpha", Some(SkillLifecycleStatus::Active));
    let error = engine.archive(&active, false).expect_err("must fail");
    assert_eq!(
        error.to_string(),
        "archive_canonical_source requires confirmed=true"
    );
}

#[test]
fn restore_archived_skill_returns_it_back_to_active_global() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    write_skill(
        &engine
            .environment()
            .home_directory
            .join(".claude")
            .join("skills"),
        "alpha",
        "# A",
    );

    let _ = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let active = find_skill(&engine, "alpha", Some(SkillLifecycleStatus::Active));
    let _ = engine.archive(&active, true).expect("archive");

    let archived = find_skill(&engine, "alpha", Some(SkillLifecycleStatus::Archived));
    let state = engine.restore(&archived, true).expect("restore");
    let restored = state
        .skills
        .iter()
        .find(|item| item.skill_key == "alpha" && item.status == SkillLifecycleStatus::Active)
        .expect("active after restore");

    assert_eq!(restored.scope, "global");
    assert!(restored
        .canonical_source_path
        .contains("/.claude/skills/alpha"));
}

#[test]
fn delete_active_skill_moves_payload_to_trash_and_removes_state_entry() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    write_skill(
        &engine
            .environment()
            .home_directory
            .join(".claude")
            .join("skills"),
        "alpha",
        "# A",
    );

    let _ = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let active = find_skill(&engine, "alpha", Some(SkillLifecycleStatus::Active));

    let state = engine.delete(&active, true).expect("delete");
    assert!(!state.skills.iter().any(|item| item.skill_key == "alpha"));

    let trash = engine.environment().home_directory.join(".Trash");
    let has_alpha = fs::read_dir(&trash)
        .expect("trash dir")
        .filter_map(Result::ok)
        .any(|entry| entry.file_name().to_string_lossy().starts_with("alpha"));
    assert!(has_alpha);
}

#[test]
fn delete_archived_skill_removes_bundle_and_state_entry() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    write_skill(
        &engine
            .environment()
            .home_directory
            .join(".claude")
            .join("skills"),
        "alpha",
        "# A",
    );

    let _ = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let active = find_skill(&engine, "alpha", Some(SkillLifecycleStatus::Active));
    let _ = engine.archive(&active, true).expect("archive");
    let archived = find_skill(&engine, "alpha", Some(SkillLifecycleStatus::Archived));

    let state = engine.delete(&archived, true).expect("delete archived");
    assert!(!state.skills.iter().any(|item| item.skill_key == "alpha"));

    let archives = engine.environment().runtime_directory.join("archives");
    let bundle_count = fs::read_dir(&archives)
        .expect("archives dir")
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy() != "subagents")
        .count();
    assert_eq!(bundle_count, 0);
}

#[test]
fn make_global_moves_project_skill_to_global_scope() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    let workspace = engine
        .environment()
        .home_directory
        .join("Dev")
        .join("workspace-a");
    write_skill(
        &workspace.join(".claude").join("skills"),
        "project-1",
        "# P",
    );

    let _ = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let project = find_skill(&engine, "project-1", Some(SkillLifecycleStatus::Active));
    assert_eq!(project.scope, "project");

    let state = engine.make_global(&project, true).expect("make global");
    let global = state
        .skills
        .iter()
        .find(|item| item.skill_key == "project-1")
        .expect("global skill");

    assert_eq!(global.scope, "global");
    assert!(global
        .canonical_source_path
        .contains("/.claude/skills/project-1"));
    assert!(!workspace
        .join(".claude")
        .join("skills")
        .join("project-1")
        .exists());
}

#[test]
fn starred_skill_is_preserved_across_rename() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    write_skill(
        &engine
            .environment()
            .home_directory
            .join(".claude")
            .join("skills"),
        "old-key",
        "# Old",
    );
    let _ = engine.run_sync(SyncTrigger::Manual).expect("sync");

    let skill = find_skill(&engine, "old-key", Some(SkillLifecycleStatus::Active));
    let _ = engine
        .set_skill_starred(&skill.id, true)
        .expect("set starred skill");

    let state = engine.rename(&skill, "New Name").expect("rename");
    let renamed = state
        .skills
        .iter()
        .find(|item| item.skill_key == "new-name")
        .expect("renamed skill");

    let starred = engine.starred_skill_ids();
    assert_eq!(starred, vec![renamed.id.clone()]);
}

#[test]
fn starred_skill_is_preserved_across_make_global() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    let workspace = engine
        .environment()
        .home_directory
        .join("Dev")
        .join("workspace-a");
    write_skill(
        &workspace.join(".claude").join("skills"),
        "project-1",
        "# Project",
    );
    let _ = engine.run_sync(SyncTrigger::Manual).expect("sync");

    let project = find_skill(&engine, "project-1", Some(SkillLifecycleStatus::Active));
    let _ = engine
        .set_skill_starred(&project.id, true)
        .expect("set starred skill");

    let state = engine.make_global(&project, true).expect("make global");
    let global = state
        .skills
        .iter()
        .find(|item| item.skill_key == "project-1" && item.scope == "global")
        .expect("global skill");

    let starred = engine.starred_skill_ids();
    assert_eq!(starred, vec![global.id.clone()]);
}

#[test]
fn set_skill_starred_prunes_unknown_ids_and_deduplicates() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    write_skill(
        &engine
            .environment()
            .home_directory
            .join(".claude")
            .join("skills"),
        "alpha",
        "# A",
    );
    let state = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let alpha = state
        .skills
        .iter()
        .find(|item| item.skill_key == "alpha")
        .expect("alpha")
        .id
        .clone();

    fs::write(
        app_settings_path(&temp),
        format!(
            "{{\"version\":2,\"auto_migrate_to_canonical_source\":false,\"workspace_discovery_roots\":[],\"window_state\":null,\"ui_state\":{{\"sidebar_width\":null,\"scope_filter\":\"all\",\"search_text\":\"\",\"selected_skill_ids\":[],\"starred_skill_ids\":[\"missing\",\"{alpha}\",\"{alpha}\"]}}}}"
        ),
    )
    .expect("write app settings");

    let starred = engine
        .set_skill_starred(&alpha, true)
        .expect("normalize starred ids");
    assert_eq!(starred, vec![alpha.clone()]);
}

#[test]
fn run_sync_discovers_global_and_project_subagents() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    write_subagent(
        &engine
            .environment()
            .home_directory
            .join(".claude")
            .join("agents"),
        "reviewer",
        "---\nname: reviewer\ndescription: Review code\n---\n\nYou are a reviewer.",
    );

    let workspace = engine
        .environment()
        .home_directory
        .join("Dev")
        .join("workspace-a");
    write_subagent(
        &workspace.join(".cursor").join("agents"),
        "debugger",
        "---\nname: debugger\ndescription: Debug issues\n---\n\nYou are a debugger.",
    );

    let state = engine.run_sync(SyncTrigger::Manual).expect("sync");
    assert_eq!(state.subagent_summary.global_count, 1);
    assert_eq!(state.subagent_summary.project_count, 1);
    assert_eq!(state.subagents.len(), 2);
}

#[test]
fn mutate_catalog_item_archives_restores_and_deletes_subagent() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    write_subagent(
        &engine
            .environment()
            .home_directory
            .join(".agents")
            .join("subagents"),
        "reviewer",
        "---\nname: reviewer\ndescription: Review code\n---\n\nYou are a reviewer.",
    );

    let state = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let active = find_subagent(&state, "reviewer", Some(SkillLifecycleStatus::Active));

    let archived_state = engine
        .mutate_catalog_item(
            CatalogMutationAction::Archive,
            CatalogMutationTarget::Subagent {
                subagent_id: active.id.clone(),
            },
            true,
        )
        .expect("archive subagent");
    let archived = find_subagent(
        &archived_state,
        "reviewer",
        Some(SkillLifecycleStatus::Archived),
    );
    assert!(archived
        .canonical_source_path
        .contains("/runtime/archives/subagents/"));
    assert!(archived.archived_bundle_path.is_some());

    let restored_state = engine
        .mutate_catalog_item(
            CatalogMutationAction::Restore,
            CatalogMutationTarget::Subagent {
                subagent_id: archived.id.clone(),
            },
            true,
        )
        .expect("restore subagent");
    let restored = find_subagent(
        &restored_state,
        "reviewer",
        Some(SkillLifecycleStatus::Active),
    );
    assert_eq!(restored.scope, "global");
    assert_eq!(restored.workspace, None);
    assert!(restored
        .canonical_source_path
        .contains("/.agents/subagents/reviewer.md"));

    let deleted_state = engine
        .mutate_catalog_item(
            CatalogMutationAction::Delete,
            CatalogMutationTarget::Subagent {
                subagent_id: restored.id.clone(),
            },
            true,
        )
        .expect("delete subagent");
    assert!(deleted_state.subagents.is_empty());
}

#[test]
fn restore_subagent_returns_to_original_project_scope_workspace() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);
    let workspace = engine
        .environment()
        .home_directory
        .join("Dev")
        .join("workspace-a");
    let workspace_key = workspace.display().to_string();

    write_subagent(
        &workspace.join(".cursor").join("agents"),
        "reviewer",
        "---\nname: reviewer\ndescription: Review code\n---\n\nYou are a reviewer.",
    );

    let state = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let active = find_subagent(&state, "reviewer", Some(SkillLifecycleStatus::Active));
    assert_eq!(active.scope, "project");
    assert_eq!(active.workspace.as_deref(), Some(workspace_key.as_str()));

    let archived_state = engine
        .mutate_catalog_item(
            CatalogMutationAction::Archive,
            CatalogMutationTarget::Subagent {
                subagent_id: active.id.clone(),
            },
            true,
        )
        .expect("archive");
    let archived = find_subagent(
        &archived_state,
        "reviewer",
        Some(SkillLifecycleStatus::Archived),
    );

    let restored_state = engine
        .mutate_catalog_item(
            CatalogMutationAction::Restore,
            CatalogMutationTarget::Subagent {
                subagent_id: archived.id.clone(),
            },
            true,
        )
        .expect("restore");
    let restored = find_subagent(
        &restored_state,
        "reviewer",
        Some(SkillLifecycleStatus::Active),
    );
    assert_eq!(restored.scope, "project");
    assert_eq!(restored.workspace.as_deref(), Some(workspace_key.as_str()));
    assert_eq!(
        restored.canonical_source_path,
        workspace
            .join(".cursor")
            .join("agents")
            .join("reviewer.md")
            .display()
            .to_string()
    );
}

#[test]
fn run_sync_reports_conflict_for_subagents_when_hashes_differ() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    write_subagent(
        &engine
            .environment()
            .home_directory
            .join(".claude")
            .join("agents"),
        "reviewer",
        "---\nname: reviewer\ndescription: Review code\n---\n\nA",
    );
    write_subagent(
        &engine
            .environment()
            .home_directory
            .join(".cursor")
            .join("agents"),
        "reviewer",
        "---\nname: reviewer\ndescription: Review code\n---\n\nB",
    );

    let error = engine.run_sync(SyncTrigger::Manual).expect_err("must fail");
    assert!(error.to_string().contains("Detected 1 conflict"));

    let persisted = engine.load_state();
    assert_eq!(persisted.summary.conflict_count, 0);
    assert_eq!(persisted.subagent_summary.conflict_count, 1);
}

#[test]
fn run_sync_writes_codex_subagent_managed_blocks() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);
    let workspace = engine
        .environment()
        .home_directory
        .join("Dev")
        .join("workspace-a");

    write_subagent(
        &engine
            .environment()
            .home_directory
            .join(".agents")
            .join("subagents"),
        "reviewer",
        "---\nname: reviewer\ndescription: Review code\nmodel: gpt-5.3-codex\ntools: [Read, Grep]\n---\n\nGlobal reviewer instructions.",
    );
    write_subagent(
        &workspace.join(".claude").join("agents"),
        "debugger",
        "---\nname: debugger\ndescription: Debug issues\n---\n\nProject debugger instructions.",
    );

    let state = engine.run_sync(SyncTrigger::Manual).expect("sync");
    assert!(state
        .subagents
        .iter()
        .any(|item| item.subagent_key == "reviewer" && item.codex_tools_ignored));

    let global_cfg = engine
        .environment()
        .home_directory
        .join(".codex")
        .join("config.toml");
    let global_raw = fs::read_to_string(global_cfg).expect("global codex config");
    assert!(global_raw.contains("# agent-sync:subagents:begin"));
    assert!(global_raw.contains("[agents.reviewer]"));

    let project_cfg = workspace.join(".codex").join("config.toml");
    let project_raw = fs::read_to_string(project_cfg).expect("project codex config");
    assert!(project_raw.contains("# agent-sync:subagents:begin"));
    assert!(project_raw.contains("[agents.debugger]"));
}

#[test]
fn run_sync_clears_codex_subagent_managed_blocks_when_subagents_removed() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    let subagent_path = engine
        .environment()
        .home_directory
        .join(".agents")
        .join("subagents")
        .join("reviewer.md");
    write_subagent(
        &engine
            .environment()
            .home_directory
            .join(".agents")
            .join("subagents"),
        "reviewer",
        "---\nname: reviewer\ndescription: Review code\n---\n\nGlobal reviewer instructions.",
    );

    let _ = engine.run_sync(SyncTrigger::Manual).expect("first sync");
    let global_cfg = engine
        .environment()
        .home_directory
        .join(".codex")
        .join("config.toml");
    let before = fs::read_to_string(&global_cfg).expect("global codex config before");
    assert!(before.contains("[agents.reviewer]"));

    fs::remove_file(&subagent_path).expect("remove subagent");
    let state = engine.run_sync(SyncTrigger::Manual).expect("second sync");
    assert!(state.subagents.is_empty());

    let after = fs::read_to_string(global_cfg).expect("global codex config after");
    assert!(after.contains("# agent-sync:subagents:begin"));
    assert!(!after.contains("[agents.reviewer]"));
}

#[test]
fn run_sync_migrates_legacy_managed_markers_in_codex_config() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);
    let home = engine.environment().home_directory.clone();

    write_skill(&home.join(".agents").join("skills"), "alpha", "# Alpha");
    write_subagent(
        &home.join(".agents").join("subagents"),
        "reviewer",
        "---\nname: reviewer\ndescription: Review code\n---\n\nReviewer instructions.",
    );

    write_text(
        &home.join(".codex").join("config.toml"),
        r#"
# skills-sync:begin
[[skills.config]]
path = "/tmp/legacy"
enabled = true
# skills-sync:end

# skills-sync:subagents:begin
[agents.legacy]
description = "Legacy"
config_file = "agents/legacy.toml"
# skills-sync:subagents:end

# skills-sync:mcp:codex:begin
[mcp_servers.legacy]
command = "legacy"
# skills-sync:mcp:codex:end
"#,
    );

    let _ = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let raw = fs::read_to_string(home.join(".codex").join("config.toml")).expect("read codex");

    assert!(!raw.contains("# skills-sync:"));
    assert!(raw.contains("# agent-sync:begin"));
    assert!(raw.contains("# agent-sync:subagents:begin"));
    assert!(raw.contains("# agent-sync:mcp:codex:begin"));
    toml::from_str::<toml::Table>(&raw).expect("valid toml");
}

#[test]
fn run_sync_avoids_duplicate_agents_table_when_legacy_key_exists() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);
    let home = engine.environment().home_directory.clone();

    write_subagent(
        &home.join(".agents").join("subagents"),
        "reviewer",
        "---\nname: reviewer\ndescription: Review code\n---\n\nReviewer instructions.",
    );
    write_text(
        &home.join(".codex").join("config.toml"),
        r#"
# skills-sync:subagents:begin
[agents.reviewer]
description = "Legacy reviewer"
config_file = "agents/reviewer.toml"
# skills-sync:subagents:end
"#,
    );

    let _ = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let raw = fs::read_to_string(home.join(".codex").join("config.toml")).expect("read codex");

    assert_eq!(count_occurrences(&raw, "[agents.reviewer]"), 1);
    assert!(!raw.contains("# skills-sync:subagents:begin"));
    toml::from_str::<toml::Table>(&raw).expect("valid toml");
}

#[test]
fn run_sync_cleans_legacy_only_subagent_block_without_discovered_subagents() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);
    let home = engine.environment().home_directory.clone();

    write_text(
        &home.join(".codex").join("config.toml"),
        r#"
# skills-sync:subagents:begin
[agents.legacy]
description = "Legacy"
config_file = "agents/legacy.toml"
# skills-sync:subagents:end
"#,
    );

    let state = engine.run_sync(SyncTrigger::Manual).expect("sync");
    assert!(state.subagents.is_empty());

    let raw = fs::read_to_string(home.join(".codex").join("config.toml")).expect("read codex");
    assert!(raw.contains("# agent-sync:subagents:begin"));
    assert!(raw.contains("# No managed subagent entries"));
    assert!(!raw.contains("[agents.legacy]"));
    assert!(!raw.contains("# skills-sync:subagents:begin"));
    toml::from_str::<toml::Table>(&raw).expect("valid toml");
}

#[test]
fn run_sync_bootstraps_mcp_catalog_from_existing_configs() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    write_text(
        &engine
            .environment()
            .home_directory
            .join(".codex")
            .join("config.toml"),
        r#"
[mcp_servers.exa]
command = "npx"
args = ["-y", "mcp-remote@latest", "https://mcp.exa.ai/mcp"]
"#,
    );
    write_text(
        &engine
            .environment()
            .home_directory
            .join(".claude")
            .join("settings.local.json"),
        r#"{
  "mcpServers": {
    "sentry": {
      "type": "http",
      "url": "https://mcp.sentry.dev/mcp"
    }
  }
}
"#,
    );

    let state = engine.run_sync(SyncTrigger::Manual).expect("sync");
    assert_eq!(state.summary.mcp_count, 2);
    assert_eq!(state.mcp_servers.len(), 2);
    assert!(state
        .mcp_servers
        .iter()
        .any(|item| item.server_key == "exa"));
    assert!(state
        .mcp_servers
        .iter()
        .any(|item| item.server_key == "sentry"));

    let central = fs::read_to_string(
        engine
            .environment()
            .home_directory
            .join(".config")
            .join("ai-agents")
            .join("config.toml"),
    )
    .expect("read central mcp catalog");
    assert!(central.contains("# agent-sync:mcp:begin"));
    assert!(central.contains("[mcp_catalog.\"global::exa\"]"));
}

#[test]
fn set_mcp_server_enabled_updates_enabled_flags() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    write_text(
        &engine
            .environment()
            .home_directory
            .join(".codex")
            .join("config.toml"),
        r#"
[mcp_servers.exa]
command = "npx"
args = ["-y", "mcp-remote@latest", "https://mcp.exa.ai/mcp"]
"#,
    );

    let _ = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let state = engine
        .set_mcp_server_enabled("exa", McpAgent::Codex, false, None, None)
        .expect("set mcp enabled");
    let exa = find_mcp(&state, "exa", "global", None);
    assert!(!exa.enabled_by_agent.codex);
}

#[test]
fn run_sync_discovers_workspace_with_only_mcp_file() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);
    let workspace = engine
        .environment()
        .home_directory
        .join("Dev")
        .join("workspace-a");
    write_text(
        &workspace.join(".mcp.json"),
        r#"{
  "mcpServers": {
    "exa": {
      "type": "http",
      "url": "https://mcp.exa.ai/mcp"
    }
  }
}
"#,
    );

    let state = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let exa = find_mcp(
        &state,
        "exa",
        "project",
        Some(&workspace.display().to_string()),
    );
    assert_eq!(exa.server_key, "exa");
}

#[test]
fn run_sync_creates_separate_project_records_for_same_server_key_different_workspaces() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);
    let workspace_a = engine
        .environment()
        .home_directory
        .join("Dev")
        .join("workspace-a");
    let workspace_b = engine
        .environment()
        .home_directory
        .join("Dev")
        .join("workspace-b");
    write_text(
        &workspace_a.join(".mcp.json"),
        r#"{
  "mcpServers": {
    "exa": {
      "type": "http",
      "url": "https://a.exa.ai/mcp"
    }
  }
}
"#,
    );
    write_text(
        &workspace_b.join(".mcp.json"),
        r#"{
  "mcpServers": {
    "exa": {
      "type": "http",
      "url": "https://b.exa.ai/mcp"
    }
  }
}
"#,
    );

    let state = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let project_exa = state
        .mcp_servers
        .iter()
        .filter(|item| item.server_key == "exa" && item.scope == "project")
        .collect::<Vec<_>>();
    assert_eq!(project_exa.len(), 2);
    assert!(project_exa
        .iter()
        .any(|item| item.workspace.as_deref() == Some(&workspace_a.display().to_string())));
    assert!(project_exa
        .iter()
        .any(|item| item.workspace.as_deref() == Some(&workspace_b.display().to_string())));
}

#[test]
fn set_enabled_without_scope_errors_on_ambiguous_server_key() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);
    let workspace_a = engine
        .environment()
        .home_directory
        .join("Dev")
        .join("workspace-a");
    let workspace_b = engine
        .environment()
        .home_directory
        .join("Dev")
        .join("workspace-b");
    write_text(
        &workspace_a.join(".mcp.json"),
        r#"{"mcpServers":{"exa":{"type":"http","url":"https://a.exa.ai/mcp"}}}"#,
    );
    write_text(
        &workspace_b.join(".mcp.json"),
        r#"{"mcpServers":{"exa":{"type":"http","url":"https://b.exa.ai/mcp"}}}"#,
    );

    let _ = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let error = engine
        .set_mcp_server_enabled("exa", McpAgent::Claude, false, None, None)
        .expect_err("must fail");
    assert!(error.to_string().contains("ambiguous"));
}

#[test]
fn set_enabled_with_scope_workspace_updates_exact_project_record_only() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);
    let workspace_a = engine
        .environment()
        .home_directory
        .join("Dev")
        .join("workspace-a");
    let workspace_b = engine
        .environment()
        .home_directory
        .join("Dev")
        .join("workspace-b");
    write_text(
        &workspace_a.join(".mcp.json"),
        r#"{"mcpServers":{"exa":{"type":"http","url":"https://mcp.exa.ai/mcp"}}}"#,
    );
    write_text(
        &workspace_b.join(".mcp.json"),
        r#"{"mcpServers":{"exa":{"type":"http","url":"https://mcp.exa.ai/mcp"}}}"#,
    );

    let _ = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let state = engine
        .set_mcp_server_enabled(
            "exa",
            McpAgent::Claude,
            false,
            Some("project"),
            Some(&workspace_a.display().to_string()),
        )
        .expect("set enabled");
    let exa_a = find_mcp(
        &state,
        "exa",
        "project",
        Some(&workspace_a.display().to_string()),
    );
    let exa_b = find_mcp(
        &state,
        "exa",
        "project",
        Some(&workspace_b.display().to_string()),
    );
    assert!(!exa_a.enabled_by_agent.claude);
    assert!(exa_b.enabled_by_agent.claude);
}

#[test]
fn mutate_catalog_item_updates_exact_project_mcp_locator() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);
    let workspace_a = engine
        .environment()
        .home_directory
        .join("Dev")
        .join("workspace-a");
    let workspace_b = engine
        .environment()
        .home_directory
        .join("Dev")
        .join("workspace-b");
    let workspace_a_key = workspace_a.display().to_string();
    let workspace_b_key = workspace_b.display().to_string();

    write_text(
        &workspace_a.join(".mcp.json"),
        r#"{"mcpServers":{"exa":{"type":"http","url":"https://a.exa.ai/mcp"}}}"#,
    );
    write_text(
        &workspace_b.join(".mcp.json"),
        r#"{"mcpServers":{"exa":{"type":"http","url":"https://b.exa.ai/mcp"}}}"#,
    );

    let _ = engine.run_sync(SyncTrigger::Manual).expect("sync");

    let archived_state = engine
        .mutate_catalog_item(
            CatalogMutationAction::Archive,
            CatalogMutationTarget::Mcp {
                server_key: String::from("exa"),
                scope: String::from("project"),
                workspace: Some(workspace_a_key.clone()),
            },
            true,
        )
        .expect("archive mcp");
    let exa_a = find_mcp(&archived_state, "exa", "project", Some(&workspace_a_key));
    let exa_b = find_mcp(&archived_state, "exa", "project", Some(&workspace_b_key));
    assert_eq!(exa_a.status, SkillLifecycleStatus::Archived);
    assert_eq!(exa_b.status, SkillLifecycleStatus::Active);

    let restored_state = engine
        .mutate_catalog_item(
            CatalogMutationAction::Restore,
            CatalogMutationTarget::Mcp {
                server_key: String::from("exa"),
                scope: String::from("project"),
                workspace: Some(workspace_a_key.clone()),
            },
            true,
        )
        .expect("restore mcp");
    let restored = find_mcp(&restored_state, "exa", "project", Some(&workspace_a_key));
    assert_eq!(restored.status, SkillLifecycleStatus::Active);

    let deleted_state = engine
        .mutate_catalog_item(
            CatalogMutationAction::Delete,
            CatalogMutationTarget::Mcp {
                server_key: String::from("exa"),
                scope: String::from("project"),
                workspace: Some(workspace_a_key.clone()),
            },
            true,
        )
        .expect("delete mcp");
    assert!(deleted_state
        .mcp_servers
        .iter()
        .all(|item| item.workspace.as_deref() != Some(workspace_a_key.as_str())));
    assert!(deleted_state
        .mcp_servers
        .iter()
        .any(|item| item.workspace.as_deref() == Some(workspace_b_key.as_str())));
}

#[test]
fn mutate_catalog_item_errors_on_ambiguous_mcp_locator() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);
    let workspace_a = engine
        .environment()
        .home_directory
        .join("Dev")
        .join("workspace-a");
    let workspace_b = engine
        .environment()
        .home_directory
        .join("Dev")
        .join("workspace-b");

    write_text(
        &workspace_a.join(".mcp.json"),
        r#"{"mcpServers":{"exa":{"type":"http","url":"https://a.exa.ai/mcp"}}}"#,
    );
    write_text(
        &workspace_b.join(".mcp.json"),
        r#"{"mcpServers":{"exa":{"type":"http","url":"https://b.exa.ai/mcp"}}}"#,
    );

    let _ = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let error = engine
        .mutate_catalog_item(
            CatalogMutationAction::Archive,
            CatalogMutationTarget::Mcp {
                server_key: String::from("exa"),
                scope: String::from("project"),
                workspace: None,
            },
            true,
        )
        .expect_err("must fail");
    assert!(error.to_string().contains("ambiguous"));
}

#[test]
fn global_record_does_not_expose_or_apply_project_toggle() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);
    write_text(
        &engine
            .environment()
            .home_directory
            .join(".codex")
            .join("config.toml"),
        r#"
[mcp_servers.exa]
command = "npx"
args = ["-y", "mcp-remote@latest", "https://mcp.exa.ai/mcp"]
"#,
    );

    let state = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let exa = find_mcp(&state, "exa", "global", None);
    assert!(!exa.enabled_by_agent.project);

    let error = engine
        .set_mcp_server_enabled("exa", McpAgent::Project, false, Some("global"), None)
        .expect_err("must fail");
    assert!(error.to_string().contains("global"));
}

#[test]
fn project_effective_flags_use_shared_project_gate() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);
    let workspace = engine
        .environment()
        .home_directory
        .join("Dev")
        .join("workspace-a");
    write_text(
        &workspace.join(".mcp.json"),
        r#"{"mcpServers":{"exa":{"type":"http","url":"https://mcp.exa.ai/mcp"}}}"#,
    );
    write_text(
        &workspace.join(".codex").join("config.toml"),
        "\n# custom codex config\n",
    );

    let _ = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let state = engine
        .set_mcp_server_enabled(
            "exa",
            McpAgent::Project,
            false,
            Some("project"),
            Some(&workspace.display().to_string()),
        )
        .expect("toggle project gate");
    let exa = find_mcp(
        &state,
        "exa",
        "project",
        Some(&workspace.display().to_string()),
    );
    assert!(!exa.enabled_by_agent.project);
    assert!(!exa
        .targets
        .iter()
        .any(|path| path == &workspace.join(".mcp.json").display().to_string()));
    assert!(!exa.targets.iter().any(|path| path
        == &workspace
            .join(".codex")
            .join("config.toml")
            .display()
            .to_string()));
}

#[test]
fn missing_project_target_emits_warning_and_skips_write() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);
    let workspace = engine
        .environment()
        .home_directory
        .join("Dev")
        .join("workspace-a");
    write_text(
        &workspace.join(".mcp.json"),
        r#"{"mcpServers":{"exa":{"type":"http","url":"https://mcp.exa.ai/mcp"}}}"#,
    );

    let _ = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let state = engine
        .set_mcp_server_enabled(
            "exa",
            McpAgent::Codex,
            true,
            Some("project"),
            Some(&workspace.display().to_string()),
        )
        .expect("enable project codex");

    assert!(state
        .sync
        .warnings
        .iter()
        .any(|item| item.contains("does not exist") && item.contains("config.toml")));
    assert!(state.summary.mcp_warning_count > 0);
    assert_eq!(state.summary.mcp_warning_count, state.sync.warnings.len());
    assert!(!workspace.join(".codex").join("config.toml").exists());
}

#[test]
fn run_sync_bootstraps_from_claude_user_root_json() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    write_text(
        &engine.environment().home_directory.join(".claude.json"),
        r#"{
  "mcpServers": {
    "exa": {
      "type": "http",
      "url": "https://mcp.exa.ai/mcp"
    }
  }
}
"#,
    );

    let state = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let exa = find_mcp(&state, "exa", "global", None);
    assert!(exa.enabled_by_agent.claude);
    assert!(exa.targets.iter().any(|path| path
        == &engine
            .environment()
            .home_directory
            .join(".claude.json")
            .display()
            .to_string()));
}

#[test]
fn run_sync_bootstraps_from_claude_user_projects_json() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);
    let workspace = engine
        .environment()
        .home_directory
        .join("Dev")
        .join("workspace-a");
    let workspace_key = workspace.display().to_string();

    write_text(
        &engine.environment().home_directory.join(".claude.json"),
        &format!(
            r#"{{
  "projects": {{
    "{workspace_key}": {{
      "mcpServers": {{
        "psnprices-prod-db": {{
          "type": "stdio",
          "command": "/tmp/mcp-prod-db"
        }}
      }}
    }}
  }}
}}
"#
        ),
    );

    let state = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let server = find_mcp(&state, "psnprices-prod-db", "project", Some(&workspace_key));
    assert!(server.enabled_by_agent.claude);
    assert!(server.enabled_by_agent.project);
    assert!(server.targets.iter().any(|path| path
        == &engine
            .environment()
            .home_directory
            .join(".claude.json")
            .display()
            .to_string()));
}

#[test]
fn workspace_mcp_json_takes_precedence_over_claude_projects_for_same_locator() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);
    let workspace = engine
        .environment()
        .home_directory
        .join("Dev")
        .join("workspace-a");
    let workspace_key = workspace.display().to_string();

    write_text(
        &workspace.join(".mcp.json"),
        r#"{
  "mcpServers": {
    "exa": {
      "type": "http",
      "url": "https://workspace.example/mcp"
    }
  }
}
"#,
    );
    write_text(
        &engine.environment().home_directory.join(".claude.json"),
        &format!(
            r#"{{
  "projects": {{
    "{workspace_key}": {{
      "mcpServers": {{
        "exa": {{
          "type": "http",
          "url": "https://claude.example/mcp"
        }}
      }}
    }}
  }}
}}
"#
        ),
    );

    let state = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let exa = find_mcp(&state, "exa", "project", Some(&workspace_key));
    assert_eq!(exa.url.as_deref(), Some("https://workspace.example/mcp"));
    assert!(exa
        .targets
        .iter()
        .any(|path| path == &workspace.join(".mcp.json").display().to_string()));
}

#[test]
fn set_enabled_updates_project_entry_in_claude_json_projects() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);
    let workspace = engine
        .environment()
        .home_directory
        .join("Dev")
        .join("workspace-a");
    let workspace_key = workspace.display().to_string();
    let claude_user = engine.environment().home_directory.join(".claude.json");

    write_text(
        &claude_user,
        &format!(
            r#"{{
  "projects": {{
    "{workspace_key}": {{
      "mcpServers": {{
        "psnprices-prod-db": {{
          "type": "stdio",
          "command": "/tmp/mcp-prod-db"
        }}
      }}
    }}
  }}
}}
"#
        ),
    );

    let _ = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let state = engine
        .set_mcp_server_enabled(
            "psnprices-prod-db",
            McpAgent::Claude,
            false,
            Some("project"),
            Some(&workspace_key),
        )
        .expect("disable project claude");
    let server = find_mcp(&state, "psnprices-prod-db", "project", Some(&workspace_key));
    assert!(!server.enabled_by_agent.claude);

    let disabled_raw = fs::read_to_string(&claude_user).expect("read claude user json");
    let disabled_json: JsonValue = serde_json::from_str(&disabled_raw).expect("parse json");
    assert!(disabled_json["projects"][&workspace_key]["mcpServers"]["psnprices-prod-db"].is_null());

    let state = engine
        .set_mcp_server_enabled(
            "psnprices-prod-db",
            McpAgent::Claude,
            true,
            Some("project"),
            Some(&workspace_key),
        )
        .expect("enable project claude");
    let server = find_mcp(&state, "psnprices-prod-db", "project", Some(&workspace_key));
    assert!(server.enabled_by_agent.claude);

    let enabled_raw = fs::read_to_string(&claude_user).expect("read claude user json");
    let enabled_json: JsonValue = serde_json::from_str(&enabled_raw).expect("parse json");
    assert_eq!(
        enabled_json["projects"][&workspace_key]["mcpServers"]["psnprices-prod-db"]["command"]
            .as_str(),
        Some("/tmp/mcp-prod-db")
    );
}

#[test]
fn global_claude_target_prefers_claude_json_over_settings_local() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    write_text(
        &engine.environment().home_directory.join(".claude.json"),
        r#"{
  "mcpServers": {
    "exa": {
      "type": "http",
      "url": "https://mcp.exa.ai/mcp"
    }
  }
}
"#,
    );
    write_text(
        &engine
            .environment()
            .home_directory
            .join(".claude")
            .join("settings.local.json"),
        r#"{
  "mcpServers": {}
}
"#,
    );

    let state = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let exa = find_mcp(&state, "exa", "global", None);
    assert!(exa.targets.iter().any(|path| path
        == &engine
            .environment()
            .home_directory
            .join(".claude.json")
            .display()
            .to_string()));
    assert!(!exa.targets.iter().any(|path| {
        path == &engine
            .environment()
            .home_directory
            .join(".claude")
            .join("settings.local.json")
            .display()
            .to_string()
    }));
}

#[test]
fn fallback_to_settings_local_when_claude_json_missing() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    write_text(
        &engine
            .environment()
            .home_directory
            .join(".claude")
            .join("settings.local.json"),
        r#"{
  "mcpServers": {
    "exa": {
      "type": "http",
      "url": "https://mcp.exa.ai/mcp"
    }
  }
}
"#,
    );

    let state = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let exa = find_mcp(&state, "exa", "global", None);
    assert!(exa.targets.iter().any(|path| {
        path == &engine
            .environment()
            .home_directory
            .join(".claude")
            .join("settings.local.json")
            .display()
            .to_string()
    }));
}

#[test]
fn run_sync_auto_aligns_claude_enabled_when_observed_in_claude_user_config() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    write_text(
        &engine
            .environment()
            .home_directory
            .join(".config")
            .join("ai-agents")
            .join("config.toml"),
        r#"
# agent-sync:mcp:begin
[mcp_catalog."global::exa"]
server_key = "exa"
scope = "global"
transport = "http"
url = "https://mcp.exa.ai/mcp"
[mcp_catalog."global::exa".enabled_by_agent]
codex = false
claude = false
project = false
# agent-sync:mcp:end
"#,
    );
    write_text(
        &engine.environment().home_directory.join(".claude.json"),
        r#"{
  "mcpServers": {
    "exa": {
      "type": "http",
      "url": "https://mcp.exa.ai/mcp"
    }
  }
}
"#,
    );

    let state = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let exa = find_mcp(&state, "exa", "global", None);
    assert!(exa.enabled_by_agent.claude);

    let central = fs::read_to_string(
        engine
            .environment()
            .home_directory
            .join(".config")
            .join("ai-agents")
            .join("config.toml"),
    )
    .expect("read central");
    assert!(central.contains("[mcp_catalog.\"global::exa\".enabled_by_agent]"));
    assert!(central.contains("claude = true"));
}

#[test]
fn manifest_v2_is_readable_and_upgraded_to_v3_locators() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);
    let workspace = engine
        .environment()
        .home_directory
        .join("Dev")
        .join("workspace-a");
    let workspace_key = workspace.display().to_string();

    write_text(
        &workspace.join(".mcp.json"),
        r#"{
  "mcpServers": {
    "exa": {
      "type": "http",
      "url": "https://mcp.exa.ai/mcp"
    }
  }
}
"#,
    );
    write_text(
        &engine
            .environment()
            .runtime_directory
            .join(".mcp-sync-manifest.json"),
        &format!(
            r#"{{
  "version": 2,
  "generated_at": "2026-02-20T10:00:00.000Z",
  "targets": {{
    "{}": ["exa"]
  }}
}}
"#,
            workspace.join(".mcp.json").display()
        ),
    );

    let _ = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let manifest_raw = fs::read_to_string(
        engine
            .environment()
            .runtime_directory
            .join(".mcp-sync-manifest.json"),
    )
    .expect("read manifest");
    let manifest_json: JsonValue = serde_json::from_str(&manifest_raw).expect("parse manifest");
    assert_eq!(manifest_json["version"].as_u64(), Some(3));
    let locators = manifest_json["targets"][workspace.join(".mcp.json").display().to_string()]
        .as_array()
        .expect("locator list");
    assert!(locators
        .iter()
        .any(|value| value.as_str() == Some(&format!("project::{workspace_key}::exa"))));
}

#[test]
fn cleanup_removes_only_previous_managed_locators_on_target_switch() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);
    let home = &engine.environment().home_directory;
    let settings_local = home.join(".claude").join("settings.local.json");
    let claude_user = home.join(".claude.json");

    write_text(
        &home.join(".config").join("ai-agents").join("config.toml"),
        r#"
# agent-sync:mcp:begin
[mcp_catalog."global::exa"]
server_key = "exa"
scope = "global"
transport = "http"
url = "https://mcp.exa.ai/mcp"
[mcp_catalog."global::exa".enabled_by_agent]
codex = false
claude = true
project = false
# agent-sync:mcp:end
"#,
    );
    write_text(
        &settings_local,
        r#"{
  "mcpServers": {
    "exa": {
      "type": "http",
      "url": "https://mcp.exa.ai/mcp"
    },
    "custom-unmanaged": {
      "type": "http",
      "url": "https://custom.example/mcp"
    }
  }
}
"#,
    );

    let _ = engine.run_sync(SyncTrigger::Manual).expect("first sync");
    write_text(&claude_user, "{\n  \"mcpServers\": {}\n}\n");

    let _ = engine.run_sync(SyncTrigger::Manual).expect("second sync");
    let local_raw = fs::read_to_string(&settings_local).expect("read legacy local json");
    let local_json: JsonValue = serde_json::from_str(&local_raw).expect("parse local json");
    assert!(local_json["mcpServers"]["exa"].is_null());
    assert_eq!(
        local_json["mcpServers"]["custom-unmanaged"]["url"].as_str(),
        Some("https://custom.example/mcp")
    );

    let user_raw = fs::read_to_string(&claude_user).expect("read claude user json");
    let user_json: JsonValue = serde_json::from_str(&user_raw).expect("parse user json");
    assert_eq!(
        user_json["mcpServers"]["exa"]["url"].as_str(),
        Some("https://mcp.exa.ai/mcp")
    );
}

#[test]
fn watch_paths_include_claude_json() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);
    let watch_paths = engine.watch_paths();
    assert!(watch_paths
        .iter()
        .any(|path| path == &engine.environment().home_directory.join(".claude.json")));
}

#[test]
fn strict_dotagents_user_scope_requires_initialized_contract() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    let error = engine
        .run_dotagents_sync(DotagentsScope::User)
        .expect_err("strict user scope should fail without agents.toml");

    assert!(error.to_string().contains("user scope is not initialized"));
}

#[test]
fn strict_dotagents_project_scope_requires_agents_toml() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);
    let workspace = engine
        .environment()
        .home_directory
        .join("Dev")
        .join("workspace-a");

    write_skill(&workspace.join(".claude").join("skills"), "alpha", "# A");

    let error = engine
        .list_dotagents_skills(DotagentsScope::Project)
        .expect_err("strict project scope should fail without agents.toml");
    let message = error.to_string();

    assert!(message.contains("project scope is not initialized"));
    assert!(message.contains(&workspace.display().to_string()));
}

#[test]
fn strict_dotagents_project_scope_requires_agents_toml_for_mcp_workspace() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);
    let workspace = engine
        .environment()
        .home_directory
        .join("Dev")
        .join("workspace-mcp-only");

    write_text(
        &workspace.join(".mcp.json"),
        r#"{"mcpServers":{"exa":{"type":"http","url":"https://mcp.exa.ai/mcp"}}}"#,
    );

    let error = engine
        .list_dotagents_mcp(DotagentsScope::Project)
        .expect_err("strict project scope should fail without agents.toml");
    let message = error.to_string();

    assert!(message.contains("project scope is not initialized"));
    assert!(message.contains(&workspace.display().to_string()));
}

#[test]
fn strict_dotagents_project_scope_allows_empty_workspace_set() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    let skills = engine
        .list_dotagents_skills(DotagentsScope::Project)
        .expect("project scope without discovered workspaces should be empty");
    assert!(skills.is_empty());

    let mcp = engine
        .list_dotagents_mcp(DotagentsScope::Project)
        .expect("project mcp scope without discovered workspaces should be empty");
    assert!(mcp.is_empty());
}

#[test]
fn strict_dotagents_project_scope_write_commands_fail_without_workspace_context() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    let error = engine
        .run_dotagents_sync(DotagentsScope::Project)
        .expect_err("project write command should fail when no workspace is discovered");
    assert!(error
        .to_string()
        .contains("no project workspaces discovered"));
}

#[test]
#[cfg(unix)]
fn strict_dotagents_user_scope_empty_list_messages_return_empty_vectors() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);
    let user_contract = engine
        .environment()
        .home_directory
        .join(".agents")
        .join("agents.toml");
    write_text(&user_contract, "[skills]\n");

    let script_path = temp.path().join("dotagents");
    write_text(
        &script_path,
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "dotagents 0.10.0"
  exit 0
fi
if [ "$1" = "--user" ]; then
  shift
fi
if [ "$1" = "list" ] && [ "$2" = "--json" ]; then
  echo "No skills declared in agents.toml."
  exit 0
fi
if [ "$1" = "mcp" ] && [ "$2" = "list" ] && [ "$3" = "--json" ]; then
  echo "No MCP servers declared in agents.toml."
  exit 0
fi
echo "unexpected args: $*" >&2
exit 9
"#,
    );
    let mut perms = fs::metadata(&script_path).expect("metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms).expect("chmod");

    let _lock = dotagents_env_lock().lock().expect("lock env");
    let _env_guard = set_env_var_with_restore("AGENT_SYNC_DOTAGENTS_BIN", &script_path);

    let skills = engine
        .list_dotagents_skills(DotagentsScope::User)
        .expect("skills list");
    let mcp = engine
        .list_dotagents_mcp(DotagentsScope::User)
        .expect("mcp list");
    assert!(skills.is_empty());
    assert!(mcp.is_empty());
}

#[test]
fn run_sync_warns_on_broken_unmanaged_claude_mcp_and_keeps_file_unchanged() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);
    let home = engine.environment().home_directory.clone();
    let workspace = home.join("Dev").join("workspace-a");
    fs::create_dir_all(&workspace).expect("workspace dir");
    let workspace_key = workspace.display().to_string();
    let broken_script = home
        .join("missing")
        .join("claude-mem")
        .join("9.0.3")
        .join("index.js");

    write_text(
        &home.join(".config").join("ai-agents").join("config.toml"),
        r#"
# agent-sync:mcp:begin
[mcp_catalog."global::managed-exa"]
server_key = "managed-exa"
scope = "global"
transport = "http"
url = "https://mcp.exa.ai/mcp"
[mcp_catalog."global::managed-exa".enabled_by_agent]
codex = true
claude = false
project = false
# agent-sync:mcp:end
"#,
    );
    write_text(
        &home.join(".claude.json"),
        &format!(
            r#"{{
  "projects": {{
    "{workspace_key}": {{
      "mcpServers": {{
        "claude-mem": {{
          "type": "stdio",
          "command": "node",
          "args": ["{}"]
        }}
      }}
    }}
  }}
}}
"#,
            broken_script.display()
        ),
    );
    let before = fs::read_to_string(home.join(".claude.json")).expect("read before");

    let state = engine.run_sync(SyncTrigger::Manual).expect("sync");
    assert!(
        state.sync.warnings.iter().any(|warning| {
            warning.contains("Broken unmanaged Claude MCP 'claude-mem'")
                && warning.contains(&broken_script.display().to_string())
        }),
        "missing explicit broken unmanaged warning: {:?}",
        state.sync.warnings
    );
    assert!(!state
        .mcp_servers
        .iter()
        .any(|server| server.server_key == "claude-mem"));

    let after = fs::read_to_string(home.join(".claude.json")).expect("read after");
    assert_eq!(after, before);
}

#[test]
fn run_sync_redacts_secret_like_arg_values_in_mcp_warnings() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);

    write_text(
        &engine
            .environment()
            .home_directory
            .join(".codex")
            .join("config.toml"),
        r#"
[mcp_servers.exa]
command = "npx"
args = ["--foo_token=super-secret-token", "--ok=1"]
"#,
    );

    let state = engine.run_sync(SyncTrigger::Manual).expect("sync");
    let exa = find_mcp(&state, "exa", "global", None);
    let joined_record = exa.warnings.join("\n");
    let joined_sync = state.sync.warnings.join("\n");

    assert!(joined_record.contains("--foo_token=<redacted>"));
    assert!(!joined_record.contains("super-secret-token"));
    assert!(joined_sync.contains("--foo_token=<redacted>"));
    assert!(!joined_sync.contains("super-secret-token"));
}

#[test]
fn fix_unmanaged_claude_mcp_dry_run_reports_candidates_without_writing() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);
    let home = engine.environment().home_directory.clone();
    let workspace = home.join("Dev").join("workspace-a");
    fs::create_dir_all(&workspace).expect("workspace dir");
    let workspace_key = workspace.display().to_string();
    let broken_global = home.join("missing").join("broken-global.js");
    let broken_project = home.join("missing").join("broken-project.js");

    write_text(
        &home.join(".config").join("ai-agents").join("config.toml"),
        r#"
# agent-sync:mcp:begin
[mcp_catalog."global::managed-keep"]
server_key = "managed-keep"
scope = "global"
transport = "stdio"
command = "node"
args = ["/tmp/missing-managed.js"]
[mcp_catalog."global::managed-keep".enabled_by_agent]
codex = false
claude = true
project = false
# agent-sync:mcp:end
"#,
    );
    write_text(
        &home.join(".claude.json"),
        &format!(
            r#"{{
  "mcpServers": {{
    "managed-keep": {{
      "type": "stdio",
      "command": "node",
      "args": ["/tmp/missing-managed.js"]
    }},
    "broken-global": {{
      "type": "stdio",
      "command": "node",
      "args": ["{}"]
    }},
    "healthy": {{
      "type": "stdio",
      "command": "node",
      "args": ["relative-script.js"]
    }}
  }},
  "projects": {{
    "{workspace_key}": {{
      "mcpServers": {{
        "broken-project": {{
          "type": "stdio",
          "command": "node",
          "args": ["{}"]
        }}
      }}
    }}
  }}
}}
"#,
            broken_global.display(),
            broken_project.display()
        ),
    );
    let before = fs::read_to_string(home.join(".claude.json")).expect("read before");

    let report = engine
        .fix_unmanaged_claude_mcp(false)
        .expect("dry run report");
    assert!(!report.apply);
    assert_eq!(report.removed_count, 0);
    assert!(report.changed_files.is_empty());
    assert_eq!(report.candidates.len(), 2);
    assert!(report
        .candidates
        .iter()
        .any(|item| item.server_key == "broken-global"));
    assert!(report
        .candidates
        .iter()
        .any(|item| item.server_key == "broken-project"));
    assert!(!report
        .candidates
        .iter()
        .any(|item| item.server_key == "managed-keep"));

    let after = fs::read_to_string(home.join(".claude.json")).expect("read after");
    assert_eq!(after, before);
}

#[test]
fn fix_unmanaged_claude_mcp_apply_removes_only_broken_unmanaged_entries() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);
    let home = engine.environment().home_directory.clone();
    let workspace = home.join("Dev").join("workspace-a");
    fs::create_dir_all(&workspace).expect("workspace dir");
    let workspace_key = workspace.display().to_string();
    let broken_global = home.join("missing").join("broken-global.js");
    let broken_project = home.join("missing").join("broken-project.js");

    write_text(
        &home.join(".config").join("ai-agents").join("config.toml"),
        r#"
# agent-sync:mcp:begin
[mcp_catalog."global::managed-keep"]
server_key = "managed-keep"
scope = "global"
transport = "stdio"
command = "node"
args = ["/tmp/missing-managed.js"]
[mcp_catalog."global::managed-keep".enabled_by_agent]
codex = false
claude = true
project = false
# agent-sync:mcp:end
"#,
    );
    write_text(
        &home.join(".claude.json"),
        &format!(
            r#"{{
  "mcpServers": {{
    "managed-keep": {{
      "type": "stdio",
      "command": "node",
      "args": ["/tmp/missing-managed.js"]
    }},
    "broken-global": {{
      "type": "stdio",
      "command": "node",
      "args": ["{}"]
    }},
    "healthy": {{
      "type": "stdio",
      "command": "node",
      "args": ["relative-script.js"]
    }}
  }},
  "projects": {{
    "{workspace_key}": {{
      "mcpServers": {{
        "broken-project": {{
          "type": "stdio",
          "command": "node",
          "args": ["{}"]
        }},
        "healthy-project": {{
          "type": "stdio",
          "command": "node",
          "args": ["relative-project.js"]
        }}
      }}
    }}
  }}
}}
"#,
            broken_global.display(),
            broken_project.display()
        ),
    );

    let report = engine.fix_unmanaged_claude_mcp(true).expect("apply report");
    assert!(report.apply);
    assert_eq!(report.removed_count, 2);
    assert_eq!(report.changed_files.len(), 1);
    assert!(report
        .changed_files
        .iter()
        .any(|path| path == &home.join(".claude.json").display().to_string()));

    let after_raw = fs::read_to_string(home.join(".claude.json")).expect("read after");
    let after_json: JsonValue = serde_json::from_str(&after_raw).expect("parse json");
    assert!(after_json["mcpServers"]["broken-global"].is_null());
    assert!(
        after_json["projects"][workspace_key.clone()]["mcpServers"]["broken-project"].is_null()
    );
    assert!(after_json["mcpServers"]["managed-keep"].is_object());
    assert!(after_json["mcpServers"]["healthy"].is_object());
    assert!(after_json["projects"][workspace_key]["mcpServers"]["healthy-project"].is_object());
}

#[test]
#[cfg(unix)]
fn strict_dotagents_mcp_unknown_command_does_not_fallback_to_skills() {
    let temp = TempDir::new().expect("tempdir");
    let engine = engine_in_temp(&temp);
    let user_contract = engine
        .environment()
        .home_directory
        .join(".agents")
        .join("agents.toml");
    write_text(&user_contract, "[skills]\n");

    let script_path = temp.path().join("dotagents");
    write_text(
        &script_path,
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "dotagents 0.10.0"
  exit 0
fi
if [ "$1" = "--user" ]; then
  shift
fi
if [ "$1" = "mcp" ] && [ "$2" = "list" ] && [ "$3" = "--json" ]; then
  echo "unknown command: mcp" >&2
  exit 2
fi
if [ "$1" = "list" ] && [ "$2" = "--json" ]; then
  echo '[{"name":"skill-entry"}]'
  exit 0
fi
echo "unexpected args: $*" >&2
exit 9
"#,
    );
    let mut perms = fs::metadata(&script_path).expect("metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms).expect("chmod");

    let _lock = dotagents_env_lock().lock().expect("lock env");
    let _env_guard = set_env_var_with_restore("AGENT_SYNC_DOTAGENTS_BIN", &script_path);

    let error = engine
        .list_dotagents_mcp(DotagentsScope::User)
        .expect_err("unknown mcp command should fail instead of falling back to skills list");
    assert!(error.to_string().contains("unknown command"));
}
