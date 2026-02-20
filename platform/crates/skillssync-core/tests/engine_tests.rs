use skillssync_core::{
    ScopeFilter, SkillLifecycleStatus, SkillLocator, SyncEngine, SyncEngineEnvironment, SyncPaths,
    SyncPreferencesStore, SyncStateStore, SyncTrigger,
};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn write_skill(root: &Path, key: &str, body: &str) {
    let skill_path = root.join(key).join("SKILL.md");
    fs::create_dir_all(skill_path.parent().expect("parent")).expect("create parent");
    fs::write(skill_path, body).expect("write skill");
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
    assert!(error.to_string().contains("Detected 1 skill conflict"));
    assert_eq!(
        engine.load_state().sync.status,
        skillssync_core::SyncHealthStatus::Failed
    );
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
        .any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("alpha")
        });
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
    write_skill(&workspace.join(".claude").join("skills"), "project-1", "# P");

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
