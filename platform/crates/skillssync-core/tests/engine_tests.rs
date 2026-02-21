use serde_json::Value as JsonValue;
use skillssync_core::{
    AuditEventStatus, McpAgent, ScopeFilter, SkillLifecycleStatus, SkillLocator, SyncEngine,
    SyncEngineEnvironment, SyncPaths, SyncPreferencesStore, SyncStateStore, SyncTrigger,
};
use std::fs;
use std::path::Path;
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

fn find_skill(
    engine: &SyncEngine,
    skill_key: &str,
    status: Option<SkillLifecycleStatus>,
) -> skillssync_core::SkillRecord {
    engine
        .find_skill(&SkillLocator {
            skill_key: String::from(skill_key),
            status,
        })
        .expect("skill exists")
}

fn find_mcp<'a>(
    state: &'a skillssync_core::SyncState,
    server_key: &str,
    scope: &str,
    workspace: Option<&str>,
) -> &'a skillssync_core::McpServerRecord {
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
    let metadata = fs::symlink_metadata(&conflicting_agents_skill).expect("managed target metadata");
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
        skillssync_core::SyncHealthStatus::Failed
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
    assert!(global_raw.contains("# skills-sync:subagents:begin"));
    assert!(global_raw.contains("[agents.reviewer]"));

    let project_cfg = workspace.join(".codex").join("config.toml");
    let project_raw = fs::read_to_string(project_cfg).expect("project codex config");
    assert!(project_raw.contains("# skills-sync:subagents:begin"));
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
    assert!(after.contains("# skills-sync:subagents:begin"));
    assert!(!after.contains("[agents.reviewer]"));
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
    assert!(central.contains("# skills-sync:mcp:begin"));
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
# skills-sync:mcp:begin
[mcp_catalog."global::exa"]
server_key = "exa"
scope = "global"
transport = "http"
url = "https://mcp.exa.ai/mcp"
[mcp_catalog."global::exa".enabled_by_agent]
codex = false
claude = false
project = false
# skills-sync:mcp:end
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
# skills-sync:mcp:begin
[mcp_catalog."global::exa"]
server_key = "exa"
scope = "global"
transport = "http"
url = "https://mcp.exa.ai/mcp"
[mcp_catalog."global::exa".enabled_by_agent]
codex = false
claude = true
project = false
# skills-sync:mcp:end
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
