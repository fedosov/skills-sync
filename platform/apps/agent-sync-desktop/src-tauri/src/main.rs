#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_runtime;
mod catalog_support;
mod command_support;
mod commands;
mod details_support;

use agent_sync_core::SyncEngine;
use app_runtime::AppRuntime;
use commands::*;
use tauri::Manager;

fn main() {
    let runtime_state = AppRuntime::default();
    tauri::Builder::default()
        .manage(runtime_state)
        .setup(|app| {
            let runtime = app.state::<AppRuntime>();
            let engine = SyncEngine::current();
            if engine.allow_filesystem_changes() {
                if let Err(error) = runtime.enable_auto_watch_and_initial_sync(&engine) {
                    eprintln!("failed to start auto watch on startup: {error}");
                    let _ = engine.set_allow_filesystem_changes(false);
                    runtime.stop_auto_watch();
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            run_sync,
            get_runtime_controls,
            set_allow_filesystem_changes,
            list_audit_events,
            clear_audit_events,
            get_state,
            get_agents_context_report,
            get_starred_skill_ids,
            set_skill_starred,
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
            set_mcp_server_enabled,
            fix_sync_warning,
            delete_unmanaged_mcp,
            mutate_catalog_item,
            rename_skill,
            get_skill_details,
            get_subagent_details,
            open_skill_path,
            open_subagent_path,
            get_platform_context,
            validate_configs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running agent-sync desktop app");
}

#[cfg(test)]
mod tests {
    use crate::{
        app_runtime::AppRuntime,
        catalog_support::{
            validate_catalog_mutation_target, CatalogMutationActionPayload,
            CatalogMutationRequestPayload, CatalogMutationTargetPayload, RenameSkillResponse,
        },
        command_support::ensure_write_allowed,
        details_support::{
            build_platform_context, normalize_os_name, read_skill_dir_tree, SkillDetails,
            SubagentDetails,
        },
    };
    use agent_sync_core::{
        AuditEventStatus, CatalogMutationAction, SkillLifecycleStatus, SkillRecord, SubagentRecord,
        SyncEngine, SyncEngineEnvironment, SyncPaths, SyncPreferencesStore, SyncState,
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
    fn rename_skill_response_serializes_with_state_and_backend_key() {
        let payload = RenameSkillResponse {
            state: SyncState::default(),
            renamed_skill_key: String::from("renamed-skill"),
        };
        let json = serde_json::to_value(payload).expect("serialize rename payload");

        assert_eq!(json["renamed_skill_key"], "renamed-skill");
        assert!(json.get("state").is_some());
    }

    #[test]
    fn detail_payloads_omit_trimmed_fields() {
        let skill_details = SkillDetails {
            skill: sample_skill_record(),
            main_file_path: String::from("/tmp/alpha/SKILL.md"),
            main_file_exists: true,
            main_file_body_preview: Some(String::from("# Preview")),
            skill_dir_tree_preview: Some(String::from("alpha/\n`-- SKILL.md")),
            last_modified_unix_seconds: Some(1_700_000_000),
        };
        let subagent_details = SubagentDetails {
            subagent: sample_subagent_record(),
            main_file_path: String::from("/tmp/reviewer.md"),
            main_file_exists: true,
            main_file_body_preview: Some(String::from("# Reviewer")),
            last_modified_unix_seconds: Some(1_700_000_000),
        };

        let skill_json = serde_json::to_value(skill_details).expect("serialize skill details");
        let subagent_json =
            serde_json::to_value(subagent_details).expect("serialize subagent details");

        assert!(skill_json.get("main_file_body_preview_truncated").is_none());
        assert!(skill_json.get("skill_dir_tree_preview_truncated").is_none());
        assert!(subagent_json
            .get("main_file_body_preview_truncated")
            .is_none());
        assert!(subagent_json.get("subagent_dir_tree_preview").is_none());
        assert!(subagent_json
            .get("subagent_dir_tree_preview_truncated")
            .is_none());
        assert!(subagent_json.get("target_statuses").is_none());
    }

    #[test]
    pub(crate) fn ensure_write_allowed_does_not_append_blocked_audit() {
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
    pub(crate) fn ensure_write_allowed_blocks_mutate_catalog_item_when_writes_disabled() {
        let temp = tempdir().expect("create tempdir");
        let engine = engine_in_temp(&temp);
        engine
            .set_allow_filesystem_changes(false)
            .expect("disable filesystem changes");

        let error = ensure_write_allowed(&engine, "mutate_catalog_item").expect_err("blocked");
        assert!(error.contains("mutate_catalog_item"));
    }

    #[test]
    pub(crate) fn validate_catalog_mutation_target_checks_mcp_payload_scope() {
        let invalid_scope = CatalogMutationTargetPayload::Mcp {
            server_key: String::from("exa"),
            scope: String::from("invalid"),
            workspace: None,
        };
        assert!(validate_catalog_mutation_target(&invalid_scope).is_err());

        let invalid_global_workspace = CatalogMutationTargetPayload::Mcp {
            server_key: String::from("exa"),
            scope: String::from("global"),
            workspace: Some(String::from("/tmp/workspace")),
        };
        assert!(validate_catalog_mutation_target(&invalid_global_workspace).is_err());

        let invalid_project_workspace = CatalogMutationTargetPayload::Mcp {
            server_key: String::from("exa"),
            scope: String::from("project"),
            workspace: None,
        };
        assert!(validate_catalog_mutation_target(&invalid_project_workspace).is_err());

        let valid_project = CatalogMutationTargetPayload::Mcp {
            server_key: String::from("exa"),
            scope: String::from("project"),
            workspace: Some(String::from("/tmp/workspace")),
        };
        assert!(validate_catalog_mutation_target(&valid_project).is_ok());
    }

    #[test]
    pub(crate) fn validate_catalog_mutation_target_checks_skill_and_subagent_keys() {
        let empty_skill = CatalogMutationTargetPayload::Skill {
            skill_key: String::new(),
        };
        assert!(validate_catalog_mutation_target(&empty_skill).is_err());

        let empty_subagent = CatalogMutationTargetPayload::Subagent {
            subagent_id: String::from("   "),
        };
        assert!(validate_catalog_mutation_target(&empty_subagent).is_err());

        let valid_skill = CatalogMutationTargetPayload::Skill {
            skill_key: String::from("alpha"),
        };
        let valid_subagent = CatalogMutationTargetPayload::Subagent {
            subagent_id: String::from("subagent-id"),
        };
        assert!(validate_catalog_mutation_target(&valid_skill).is_ok());
        assert!(validate_catalog_mutation_target(&valid_subagent).is_ok());
    }

    #[test]
    fn catalog_mutation_payload_supports_make_global_action() {
        let payload: CatalogMutationRequestPayload = serde_json::from_str(
            r#"{
  "action": "make_global",
  "target": {
    "kind": "mcp",
    "serverKey": "exa",
    "scope": "project",
    "workspace": "/tmp/workspace-a"
  },
  "confirmed": true
}"#,
        )
        .expect("deserialize make_global payload");

        assert!(matches!(
            payload.action,
            CatalogMutationActionPayload::MakeGlobal
        ));
        assert_eq!(
            CatalogMutationAction::from(payload.action),
            CatalogMutationAction::MakeGlobal
        );
    }

    fn sample_skill_record() -> SkillRecord {
        SkillRecord {
            id: String::from("skill-1"),
            name: String::from("Skill"),
            scope: String::from("global"),
            workspace: None,
            canonical_source_path: String::from("/tmp/alpha"),
            target_paths: vec![String::from("/tmp/alpha")],
            exists: true,
            is_symlink_canonical: false,
            package_type: String::from("dir"),
            skill_key: String::from("alpha"),
            symlink_target: String::from("/tmp/alpha"),
            source: None,
            commit: None,
            install_status: None,
            wildcard_source: None,
            status: SkillLifecycleStatus::Active,
            archived_at: None,
            archived_bundle_path: None,
            archived_original_scope: None,
            archived_original_workspace: None,
        }
    }

    fn sample_subagent_record() -> SubagentRecord {
        SubagentRecord {
            id: String::from("subagent-1"),
            name: String::from("Reviewer"),
            description: String::from("Review code"),
            scope: String::from("global"),
            workspace: None,
            canonical_source_path: String::from("/tmp/reviewer.md"),
            target_paths: vec![String::from("/tmp/reviewer.md")],
            exists: true,
            is_symlink_canonical: false,
            package_type: String::from("file"),
            subagent_key: String::from("reviewer"),
            symlink_target: String::from("/tmp/reviewer.md"),
            model: None,
            tools: Vec::new(),
            codex_tools_ignored: false,
            status: SkillLifecycleStatus::Active,
            archived_at: None,
            archived_bundle_path: None,
            archived_original_scope: None,
            archived_original_workspace: None,
        }
    }

    #[test]
    fn enable_auto_watch_waits_for_initial_sync_before_returning() {
        let temp = tempdir().expect("create tempdir");
        let engine = engine_in_temp(&temp);
        engine
            .set_allow_filesystem_changes(true)
            .expect("enable filesystem changes");
        let runtime = AppRuntime::default();

        let (lock_acquired_tx, lock_acquired_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let held_runtime = runtime.clone();
        let hold_join = std::thread::spawn(move || {
            held_runtime
                .with_sync_lock(|| {
                    lock_acquired_tx
                        .send(())
                        .expect("signal held sync lock acquired");
                    release_rx
                        .recv_timeout(Duration::from_secs(30))
                        .map_err(|_| String::from("timed out waiting to release sync lock"))?;
                    Ok(())
                })
                .expect("hold sync lock")
        });
        lock_acquired_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("sync lock should be held");
        let (tx, rx) = mpsc::channel();
        let runtime_for_thread = runtime.clone();
        let engine_for_thread = engine.clone();
        let join = std::thread::spawn(move || {
            let result = runtime_for_thread.enable_auto_watch_and_initial_sync(&engine_for_thread);
            tx.send(result).expect("send startup result");
        });

        assert!(
            rx.recv_timeout(Duration::from_secs(1)).is_err(),
            "startup should wait for initial sync lock"
        );

        release_tx
            .send(())
            .expect("release held sync lock for startup");
        let result = rx
            .recv_timeout(Duration::from_secs(30))
            .expect("startup should finish once lock is released");
        assert!(result.is_ok());

        runtime.stop_auto_watch();
        join.join().expect("join startup thread");
        hold_join.join().expect("join sync-lock holder");
    }

    #[test]
    fn set_allow_filesystem_changes_reports_startup_errors_and_reverts_writes() {
        let temp = tempdir().expect("create tempdir");
        let engine = engine_in_temp(&temp);
        let runtime = AppRuntime::default();

        let result = runtime.set_allow_filesystem_changes_with(&engine, true, |_r, _e| {
            Err(String::from("watch startup failed"))
        });

        assert!(matches!(result, Err(error) if error == "watch startup failed"));
        assert!(!engine.allow_filesystem_changes());
        assert!(ensure_write_allowed(&engine, "mutate_catalog_item").is_err());
    }

    #[test]
    fn set_allow_filesystem_changes_reverts_write_mode_when_initial_sync_fails() {
        let temp = tempdir().expect("create tempdir");
        let engine = engine_in_temp(&temp);
        let runtime = AppRuntime::default();
        let home = engine.environment().home_directory.clone();

        let claude_skill = home.join(".claude").join("skills").join("duplicate");
        fs::create_dir_all(&claude_skill).expect("create claude skill dir");
        fs::write(claude_skill.join("SKILL.md"), "# A").expect("write claude skill");

        let agents_skill = home.join(".agents").join("skills").join("duplicate");
        fs::create_dir_all(&agents_skill).expect("create agents skill dir");
        fs::write(agents_skill.join("SKILL.md"), "# B").expect("write agents skill");

        let result = runtime.set_allow_filesystem_changes(&engine, true);

        assert!(matches!(result, Err(error) if error.contains("Detected 1 conflict")));
        assert!(!engine.allow_filesystem_changes());
        assert!(ensure_write_allowed(&engine, "mutate_catalog_item").is_err());
    }
}
