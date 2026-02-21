use crate::codex_registry::CodexSkillsRegistryWriter;
use crate::codex_subagent_registry::{CodexSubagentConfigEntry, CodexSubagentRegistryWriter};
use crate::error::SyncEngineError;
use crate::mcp_registry::{McpAgent, McpRegistry, McpSyncOutcome};
use crate::models::{
    McpServerRecord, SkillLifecycleStatus, SkillRecord, SubagentRecord, SyncConflict,
    SyncConflictKind, SyncHealthStatus, SyncMetadata, SyncState, SyncSummary, SyncTrigger,
};
use crate::paths::{home_dir, SyncPaths};
use crate::settings::{AppUiState, SyncPreferencesStore};
use crate::state_store::SyncStateStore;
use crate::watch::default_watch_paths;
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sha1::{Digest as Sha1Digest, Sha1};
use sha2::Sha256;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use uuid::Uuid;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct SyncEngineEnvironment {
    pub home_directory: PathBuf,
    pub dev_root: PathBuf,
    pub worktrees_root: PathBuf,
    pub runtime_directory: PathBuf,
}

impl SyncEngineEnvironment {
    pub fn current() -> Self {
        let home = home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let runtime_directory = std::env::var("SKILLS_SYNC_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home.join(".config").join("ai-agents").join("skillssync"));
        Self {
            home_directory: home.clone(),
            dev_root: home.join("Dev"),
            worktrees_root: home.join(".codex").join("worktrees"),
            runtime_directory,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeFilter {
    All,
    Global,
    Project,
    Archived,
}

impl ScopeFilter {
    pub fn matches(self, skill: &SkillRecord) -> bool {
        match self {
            Self::All => true,
            Self::Global => skill.status == SkillLifecycleStatus::Active && skill.scope == "global",
            Self::Project => {
                skill.status == SkillLifecycleStatus::Active && skill.scope == "project"
            }
            Self::Archived => skill.status == SkillLifecycleStatus::Archived,
        }
    }
}

impl FromStr for ScopeFilter {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "all" => Ok(Self::All),
            "global" => Ok(Self::Global),
            "project" => Ok(Self::Project),
            "archived" => Ok(Self::Archived),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SkillLocator {
    pub skill_key: String,
    pub status: Option<SkillLifecycleStatus>,
}

#[derive(Debug, Clone)]
struct SkillPackage {
    source_root: PathBuf,
    skill_key: String,
    name: String,
    canonical_path: PathBuf,
    package_type: String,
    package_hash: String,
}

#[derive(Debug, Clone)]
struct SubagentPackage {
    source_root: PathBuf,
    subagent_key: String,
    name: String,
    description: String,
    model: Option<String>,
    tools: Vec<String>,
    body: String,
    canonical_path: PathBuf,
    package_type: String,
    package_hash: String,
}

#[derive(Debug)]
struct SyncCoreResult {
    entries: Vec<SkillRecord>,
    subagent_entries: Vec<SubagentRecord>,
    codex_subagent_entries: Vec<CodexSubagentConfigEntry>,
    skill_conflict_count: usize,
    subagent_conflict_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ArchivedSkillManifest {
    version: u32,
    #[serde(rename = "archived_at")]
    archived_at: String,
    #[serde(rename = "skill_key")]
    skill_key: String,
    name: String,
    #[serde(rename = "original_scope")]
    original_scope: String,
    #[serde(rename = "original_workspace")]
    original_workspace: Option<String>,
    #[serde(rename = "original_canonical_source_path")]
    original_canonical_source_path: String,
    #[serde(rename = "moved_links")]
    moved_links: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SyncEngine {
    environment: SyncEngineEnvironment,
    store: SyncStateStore,
    preferences_store: SyncPreferencesStore,
    protected_segments: HashSet<String>,
}

impl Default for SyncEngine {
    fn default() -> Self {
        Self::current()
    }
}

impl SyncEngine {
    pub fn current() -> Self {
        let paths = SyncPaths::detect();
        Self {
            environment: SyncEngineEnvironment::current(),
            store: SyncStateStore::new(paths.clone()),
            preferences_store: SyncPreferencesStore::new(paths),
            protected_segments: HashSet::from([String::from(".system")]),
        }
    }

    pub fn new(
        environment: SyncEngineEnvironment,
        store: SyncStateStore,
        preferences_store: SyncPreferencesStore,
    ) -> Self {
        Self {
            environment,
            store,
            preferences_store,
            protected_segments: HashSet::from([String::from(".system")]),
        }
    }

    pub fn environment(&self) -> &SyncEngineEnvironment {
        &self.environment
    }

    pub fn load_state(&self) -> SyncState {
        self.store.load_state()
    }

    pub fn starred_skill_ids(&self) -> Vec<String> {
        let state = self.store.load_state();
        let settings = self.preferences_store.load_settings();
        self.sanitize_starred_skill_ids(&state, &settings)
    }

    pub fn set_skill_starred(
        &self,
        skill_id: &str,
        starred: bool,
    ) -> Result<Vec<String>, SyncEngineError> {
        let state = self.store.load_state();
        if !state.skills.iter().any(|skill| skill.id == skill_id) {
            return Ok(self.starred_skill_ids());
        }

        let settings = self.preferences_store.load_settings();
        let mut next = self.sanitize_starred_skill_ids(&state, &settings);

        if starred {
            if !next.iter().any(|id| id == skill_id) {
                next.push(skill_id.to_string());
            }
        } else {
            next.retain(|id| id != skill_id);
        }

        self.save_starred_skill_ids(settings, next.clone())?;
        Ok(next)
    }

    pub fn list_skills(&self, scope: ScopeFilter) -> Vec<SkillRecord> {
        let mut items: Vec<SkillRecord> = self
            .store
            .load_state()
            .skills
            .into_iter()
            .filter(|skill| scope.matches(skill))
            .collect();
        items.sort_by(sort_entries);
        items
    }

    pub fn list_subagents(&self, scope: ScopeFilter) -> Vec<SubagentRecord> {
        let state = self.store.load_state();
        let mut items: Vec<SubagentRecord> = state
            .subagents
            .into_iter()
            .filter(|item| match scope {
                ScopeFilter::All => true,
                ScopeFilter::Global => item.scope == "global",
                ScopeFilter::Project => item.scope == "project",
                ScopeFilter::Archived => false,
            })
            .collect();
        items.sort_by(sort_subagent_entries);
        items
    }

    pub fn list_mcp_servers(&self) -> Vec<McpServerRecord> {
        let mut items = self.store.load_state().mcp_servers;
        items.sort_by(|lhs, rhs| lhs.server_key.cmp(&rhs.server_key));
        items
    }

    pub fn set_mcp_server_enabled(
        &self,
        server_key: &str,
        agent: McpAgent,
        enabled: bool,
        scope: Option<&str>,
        workspace: Option<&str>,
    ) -> Result<SyncState, SyncEngineError> {
        let workspaces = self.workspace_candidates();
        let registry = McpRegistry::new(
            self.environment.home_directory.clone(),
            self.environment.runtime_directory.clone(),
        );
        registry.set_enabled(&workspaces, server_key, agent, enabled, scope, workspace)?;
        self.run_sync(SyncTrigger::Manual)
    }

    pub fn watch_paths(&self) -> Vec<PathBuf> {
        let mut paths = default_watch_paths(&self.environment.home_directory);
        let workspaces = self.workspace_candidates();
        let registry = McpRegistry::new(
            self.environment.home_directory.clone(),
            self.environment.runtime_directory.clone(),
        );
        paths.extend(registry.managed_watch_paths(&workspaces));

        let mut unique = HashSet::new();
        let mut deduped = Vec::new();
        for path in paths {
            let key = standardized_path(&path);
            if unique.insert(key) {
                deduped.push(path);
            }
        }
        deduped.sort();
        deduped
    }

    pub fn find_skill(&self, locator: &SkillLocator) -> Option<SkillRecord> {
        self.store.load_state().skills.into_iter().find(|skill| {
            skill.skill_key == locator.skill_key
                && locator
                    .status
                    .map(|status| skill.status == status)
                    .unwrap_or(true)
        })
    }

    pub fn find_subagent_by_id(&self, subagent_id: &str) -> Option<SubagentRecord> {
        self.store
            .load_state()
            .subagents
            .into_iter()
            .find(|item| item.id == subagent_id)
    }

    pub fn run_sync(&self, _trigger: SyncTrigger) -> Result<SyncState, SyncEngineError> {
        let started = Utc::now();
        let previous_state = self.store.load_state();

        match self.run_core_sync() {
            Ok(result) => {
                let registry_writer =
                    CodexSkillsRegistryWriter::new(self.environment.home_directory.clone());
                registry_writer
                    .write_managed_registry(&result.entries)
                    .map_err(|e| SyncEngineError::CodexRegistryWriteFailed(e.to_string()))?;
                let subagent_registry_writer =
                    CodexSubagentRegistryWriter::new(self.environment.home_directory.clone());
                subagent_registry_writer
                    .write_managed_registries(&result.codex_subagent_entries)
                    .map_err(|e| SyncEngineError::CodexRegistryWriteFailed(e.to_string()))?;
                let workspaces = self.workspace_candidates();
                let mcp_registry = McpRegistry::new(
                    self.environment.home_directory.clone(),
                    self.environment.runtime_directory.clone(),
                );
                let mcp_outcome = mcp_registry.sync(&workspaces)?;

                let finished = Utc::now();
                let state = self.make_state(
                    SyncHealthStatus::Ok,
                    result.entries,
                    result.subagent_entries,
                    mcp_outcome,
                    result.skill_conflict_count,
                    result.subagent_conflict_count,
                    started,
                    finished,
                    None,
                );
                self.store.save_state(&state)?;
                Ok(state)
            }
            Err(error) => {
                let (skill_conflict_count, subagent_conflict_count) = match &error {
                    SyncEngineError::Conflicts(_, conflicts) => {
                        conflicts
                            .iter()
                            .fold((0usize, 0usize), |(skills, subagents), item| {
                                match item.kind {
                                    SyncConflictKind::Skill => (skills + 1, subagents),
                                    SyncConflictKind::Subagent => (skills, subagents + 1),
                                }
                            })
                    }
                    _ => (0, 0),
                };
                let finished = Utc::now();
                let failed = self.make_failed_state(
                    previous_state,
                    started,
                    finished,
                    error.to_string(),
                    skill_conflict_count,
                    subagent_conflict_count,
                );
                let _ = self.store.save_state(&failed);
                Err(error)
            }
        }
    }

    pub fn delete(
        &self,
        skill: &SkillRecord,
        confirmed: bool,
    ) -> Result<SyncState, SyncEngineError> {
        if !confirmed {
            return Err(SyncEngineError::DeleteRequiresConfirmation);
        }

        let target = if skill.status == SkillLifecycleStatus::Archived {
            skill
                .archived_bundle_path
                .as_ref()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(&skill.canonical_source_path))
        } else {
            PathBuf::from(&skill.canonical_source_path)
        };

        if self.is_protected_path(&target) {
            return Err(SyncEngineError::DeletionBlockedProtectedPath);
        }

        let roots = self.allowed_delete_roots();
        if !roots.iter().any(|root| self.is_relative_to(&target, root)) {
            return Err(SyncEngineError::DeletionOutsideAllowedRoots);
        }

        if !path_exists_or_symlink(&target) {
            return Err(SyncEngineError::DeletionTargetMissing);
        }

        self.move_to_trash(&target)?;
        self.run_sync(SyncTrigger::Delete)
    }

    pub fn archive(
        &self,
        skill: &SkillRecord,
        confirmed: bool,
    ) -> Result<SyncState, SyncEngineError> {
        if !confirmed {
            return Err(SyncEngineError::ArchiveRequiresConfirmation);
        }
        if skill.status != SkillLifecycleStatus::Active {
            return Err(SyncEngineError::ArchiveOnlyForActiveSkill);
        }

        let source = PathBuf::from(&skill.canonical_source_path);
        if self.is_protected_path(&source) {
            return Err(SyncEngineError::ArchiveBlockedProtectedPath);
        }

        let roots = self.allowed_delete_roots();
        if !roots.iter().any(|root| self.is_relative_to(&source, root)) {
            return Err(SyncEngineError::ArchiveOutsideAllowedRoots);
        }
        if !path_exists_or_symlink(&source) {
            return Err(SyncEngineError::ArchiveSourceMissing);
        }

        let archived_at = iso8601_now();
        let bundle = self
            .archives_root()
            .join(self.make_archive_bundle_name(&skill.skill_key));
        let source_archive_path = bundle.join("source");
        let links_archive_path = bundle.join("links");

        fs::create_dir_all(&links_archive_path)
            .map_err(|e| SyncEngineError::io(&links_archive_path, e))?;

        fs::rename(&source, &source_archive_path).map_err(|e| SyncEngineError::io(&source, e))?;

        let mut moved_links = Vec::new();
        let mut used_link_names: HashSet<String> = HashSet::new();
        for target_path in &skill.target_paths {
            let target = PathBuf::from(target_path);
            if standardized_path(&target) == standardized_path(&source) {
                continue;
            }
            if !is_symlink(&target) {
                continue;
            }
            let archived_link = self.unique_archived_link_path(
                target
                    .file_name()
                    .unwrap_or_else(|| OsStr::new("link"))
                    .to_string_lossy()
                    .as_ref(),
                &links_archive_path,
                &mut used_link_names,
            );
            fs::rename(&target, &archived_link).map_err(|e| SyncEngineError::io(&target, e))?;
            moved_links.push(target.display().to_string());
        }

        let manifest = ArchivedSkillManifest {
            version: 1,
            archived_at,
            skill_key: skill.skill_key.clone(),
            name: skill.name.clone(),
            original_scope: skill.scope.clone(),
            original_workspace: skill.workspace.clone(),
            original_canonical_source_path: source.display().to_string(),
            moved_links,
        };

        self.write_archived_manifest(&manifest, &bundle)?;
        self.run_sync(SyncTrigger::Archive)
    }

    pub fn restore(
        &self,
        skill: &SkillRecord,
        confirmed: bool,
    ) -> Result<SyncState, SyncEngineError> {
        if !confirmed {
            return Err(SyncEngineError::RestoreRequiresConfirmation);
        }
        if skill.status != SkillLifecycleStatus::Archived {
            return Err(SyncEngineError::RestoreOnlyForArchivedSkill);
        }

        let bundle = skill
            .archived_bundle_path
            .as_ref()
            .map(PathBuf::from)
            .ok_or(SyncEngineError::RestoreBundleMissing)?;
        let source = bundle.join("source");
        if !path_exists_or_symlink(&source) {
            return Err(SyncEngineError::RestoreSourceMissing);
        }

        let manifest = self.read_archived_manifest(&bundle)?;
        let destination = self.preferred_global_destination(&manifest.skill_key);
        if path_exists_or_symlink(&destination) {
            return Err(SyncEngineError::RestoreTargetExists);
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|e| SyncEngineError::io(parent, e))?;
        }
        fs::rename(&source, &destination).map_err(|e| SyncEngineError::io(&source, e))?;
        let _ = fs::remove_dir_all(&bundle);

        self.run_sync(SyncTrigger::Restore)
    }

    pub fn make_global(
        &self,
        skill: &SkillRecord,
        confirmed: bool,
    ) -> Result<SyncState, SyncEngineError> {
        if !confirmed {
            return Err(SyncEngineError::MakeGlobalRequiresConfirmation);
        }
        if skill.scope != "project" {
            return Err(SyncEngineError::MakeGlobalOnlyForProject);
        }

        let skill_key = skill.skill_key.trim();
        if skill_key.is_empty() {
            return Err(SyncEngineError::MakeGlobalOutsideAllowedRoots);
        }
        if self.is_protected_skill_key(skill_key) {
            return Err(SyncEngineError::MakeGlobalBlockedProtectedPath);
        }

        let source = PathBuf::from(&skill.canonical_source_path);
        if self.is_protected_path(&source) {
            return Err(SyncEngineError::MakeGlobalBlockedProtectedPath);
        }

        let roots = self.allowed_project_roots();
        if !roots.iter().any(|root| self.is_relative_to(&source, root)) {
            return Err(SyncEngineError::MakeGlobalOutsideAllowedRoots);
        }
        if !path_exists_or_symlink(&source) {
            return Err(SyncEngineError::MakeGlobalSourceMissing);
        }

        let destination = self.preferred_global_destination(skill_key);
        if path_exists_or_symlink(&destination) {
            return Err(SyncEngineError::MakeGlobalTargetExists);
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|e| SyncEngineError::io(parent, e))?;
        }
        fs::rename(&source, &destination).map_err(|e| SyncEngineError::io(&source, e))?;
        let next = self.run_sync(SyncTrigger::MakeGlobal)?;
        self.migrate_starred_skill_id(
            &skill.id,
            &skill_entry_id("global", None, skill_key),
            &next,
        )?;
        Ok(next)
    }

    pub fn rename(
        &self,
        skill: &SkillRecord,
        new_title: &str,
    ) -> Result<SyncState, SyncEngineError> {
        let new_key = normalized_skill_key(new_title);
        if new_key.is_empty() {
            return Err(SyncEngineError::RenameRequiresNonEmptyTitle);
        }
        if new_key == skill.skill_key {
            return Err(SyncEngineError::RenameNoOp);
        }
        if self.is_protected_skill_key(&skill.skill_key) || self.is_protected_skill_key(&new_key) {
            return Err(SyncEngineError::RenameBlockedProtectedPath);
        }

        let source = PathBuf::from(&skill.canonical_source_path);
        if self.is_protected_path(&source) {
            return Err(SyncEngineError::RenameBlockedProtectedPath);
        }

        let roots = self.allowed_delete_roots();
        if !roots.iter().any(|root| self.is_relative_to(&source, root)) {
            return Err(SyncEngineError::RenameOutsideAllowedRoots);
        }
        if !path_exists_or_symlink(&source) {
            return Err(SyncEngineError::RenameRequiresExistingSource);
        }

        let destination = if skill.scope == "project" {
            let workspace = skill
                .workspace
                .as_ref()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
                .ok_or(SyncEngineError::RenameOutsideAllowedRoots)?;
            PathBuf::from(workspace)
                .join(".claude")
                .join("skills")
                .join(&new_key)
        } else {
            self.preferred_global_destination(&new_key)
        };

        if self.is_protected_path(&destination) {
            return Err(SyncEngineError::RenameBlockedProtectedPath);
        }
        if standardized_path(&source) == standardized_path(&destination) {
            return Err(SyncEngineError::RenameNoOp);
        }
        if path_exists_or_symlink(&destination) {
            return Err(SyncEngineError::RenameConflictTargetExists);
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|e| SyncEngineError::io(parent, e))?;
        }
        fs::rename(&source, &destination).map_err(|e| SyncEngineError::io(&source, e))?;

        let skill_file = destination.join("SKILL.md");
        if let Err(error) = update_skill_title(&skill_file, new_title.trim()) {
            let _ = fs::rename(&destination, &source);
            return Err(error);
        }

        let next = self.run_sync(SyncTrigger::Rename)?;
        self.migrate_starred_skill_id(
            &skill.id,
            &skill_entry_id(skill.scope.as_str(), skill.workspace.as_deref(), &new_key),
            &next,
        )?;
        Ok(next)
    }

    fn run_core_sync(&self) -> Result<SyncCoreResult, SyncEngineError> {
        self.ensure_directories()?;
        let settings = self.preferences_store.load_settings();

        let global_candidates = self.discover_global_packages();
        let (mut global_canonical, mut conflicts) =
            self.resolve_scope_candidates(&global_candidates, "global", None);

        let workspaces = self.workspace_candidates();
        let mut project_candidates_by_workspace: HashMap<PathBuf, Vec<SkillPackage>> =
            HashMap::new();
        let mut project_resolved_by_workspace: HashMap<PathBuf, BTreeMap<String, SkillPackage>> =
            HashMap::new();
        let global_subagent_candidates = self.discover_global_subagents();
        let (global_subagent_canonical, subagent_conflicts) =
            self.resolve_subagent_scope_candidates(&global_subagent_candidates, "global", None);
        conflicts.extend(subagent_conflicts);
        let mut project_subagent_candidates_by_workspace: HashMap<PathBuf, Vec<SubagentPackage>> =
            HashMap::new();
        let mut project_subagent_resolved_by_workspace: HashMap<
            PathBuf,
            BTreeMap<String, SubagentPackage>,
        > = HashMap::new();

        for workspace in &workspaces {
            let candidates = self.discover_project_packages(workspace);
            let (resolved, scope_conflicts) =
                self.resolve_scope_candidates(&candidates, "project", Some(workspace));
            conflicts.extend(scope_conflicts);
            project_candidates_by_workspace.insert(workspace.clone(), candidates);
            project_resolved_by_workspace.insert(workspace.clone(), resolved);

            let subagent_candidates = self.discover_project_subagents(workspace);
            let (subagent_resolved, subagent_scope_conflicts) = self
                .resolve_subagent_scope_candidates(
                    &subagent_candidates,
                    "project",
                    Some(workspace),
                );
            conflicts.extend(subagent_scope_conflicts);
            project_subagent_candidates_by_workspace.insert(workspace.clone(), subagent_candidates);
            project_subagent_resolved_by_workspace.insert(workspace.clone(), subagent_resolved);
        }

        if !conflicts.is_empty() {
            return Err(SyncEngineError::conflicts(conflicts));
        }

        if settings.auto_migrate_to_canonical_source {
            global_canonical =
                self.migrate_scope_candidates_to_claude(&global_candidates, "global", None)?;

            for workspace in &workspaces {
                let candidates = project_candidates_by_workspace
                    .get(workspace)
                    .cloned()
                    .unwrap_or_default();
                let migrated = self.migrate_scope_candidates_to_claude(
                    &candidates,
                    "project",
                    Some(workspace),
                )?;
                project_resolved_by_workspace.insert(workspace.clone(), migrated);
            }
        }

        let old_managed_links = self.load_managed_links_manifest();
        let mut new_managed_links: HashSet<String> = HashSet::new();
        let old_subagent_managed_links = self.load_subagent_managed_links_manifest();
        let mut new_subagent_managed_links: HashSet<String> = HashSet::new();
        let mut entries: Vec<SkillRecord> = Vec::new();
        let mut subagent_entries: Vec<SubagentRecord> = Vec::new();
        let mut codex_subagent_entries: Vec<CodexSubagentConfigEntry> = Vec::new();

        for (skill_key, package) in &global_canonical {
            let mut target_paths = Vec::new();
            for target_root in self.global_targets() {
                let target = target_root.join(skill_key);
                if standardized_path(&target) == standardized_path(&package.canonical_path) {
                    target_paths.push(target.display().to_string());
                    continue;
                }

                self.create_or_update_symlink(&target, &package.canonical_path)?;
                new_managed_links.insert(standardized_path(&target));
                target_paths.push(target.display().to_string());
            }
            entries.push(self.create_skill_entry("global", None, skill_key, package, target_paths));
        }

        for workspace in &workspaces {
            let Some(canonical) = project_resolved_by_workspace.get(workspace) else {
                continue;
            };
            let target_roots = self.project_targets(workspace);
            for (skill_key, package) in canonical {
                let mut target_paths = Vec::new();
                for target_root in &target_roots {
                    let target = target_root.join(skill_key);
                    if standardized_path(&target) == standardized_path(&package.canonical_path) {
                        target_paths.push(target.display().to_string());
                        continue;
                    }

                    self.create_or_update_symlink(&target, &package.canonical_path)?;
                    new_managed_links.insert(standardized_path(&target));
                    target_paths.push(target.display().to_string());
                }

                entries.push(self.create_skill_entry(
                    "project",
                    Some(workspace.display().to_string()),
                    skill_key,
                    package,
                    target_paths,
                ));
            }
        }

        for (subagent_key, package) in &global_subagent_canonical {
            let mut target_paths = Vec::new();
            for target_root in self.global_subagent_targets() {
                let target = target_root.join(format!("{subagent_key}.md"));
                if standardized_path(&target) == standardized_path(&package.canonical_path) {
                    target_paths.push(target.display().to_string());
                    continue;
                }
                self.create_or_update_symlink(&target, &package.canonical_path)?;
                new_subagent_managed_links.insert(standardized_path(&target));
                target_paths.push(target.display().to_string());
            }
            subagent_entries.push(self.create_subagent_entry(
                "global",
                None,
                subagent_key,
                package,
                target_paths,
            ));
            codex_subagent_entries.push(CodexSubagentConfigEntry {
                scope: "global".to_string(),
                workspace: None,
                subagent_key: subagent_key.clone(),
                description: package.description.clone(),
                prompt: package.body.clone(),
                model: package.model.clone(),
            });
        }

        for workspace in &workspaces {
            let Some(canonical) = project_subagent_resolved_by_workspace.get(workspace) else {
                continue;
            };
            let target_roots = self.project_subagent_targets(workspace);
            for (subagent_key, package) in canonical {
                let mut target_paths = Vec::new();
                for target_root in &target_roots {
                    let target = target_root.join(format!("{subagent_key}.md"));
                    if standardized_path(&target) == standardized_path(&package.canonical_path) {
                        target_paths.push(target.display().to_string());
                        continue;
                    }
                    self.create_or_update_symlink(&target, &package.canonical_path)?;
                    new_subagent_managed_links.insert(standardized_path(&target));
                    target_paths.push(target.display().to_string());
                }
                subagent_entries.push(self.create_subagent_entry(
                    "project",
                    Some(workspace.display().to_string()),
                    subagent_key,
                    package,
                    target_paths,
                ));
                codex_subagent_entries.push(CodexSubagentConfigEntry {
                    scope: "project".to_string(),
                    workspace: Some(workspace.display().to_string()),
                    subagent_key: subagent_key.clone(),
                    description: package.description.clone(),
                    prompt: package.body.clone(),
                    model: package.model.clone(),
                });
            }
        }

        self.cleanup_stale_links(&old_managed_links, &new_managed_links);
        self.save_managed_links_manifest(&new_managed_links)?;
        self.cleanup_stale_links(&old_subagent_managed_links, &new_subagent_managed_links);
        self.save_subagent_managed_links_manifest(&new_subagent_managed_links)?;

        entries.extend(self.load_archived_entries());
        entries.sort_by(sort_entries);
        subagent_entries.sort_by(sort_subagent_entries);

        Ok(SyncCoreResult {
            entries,
            subagent_entries,
            codex_subagent_entries,
            skill_conflict_count: 0,
            subagent_conflict_count: 0,
        })
    }

    fn create_skill_entry(
        &self,
        scope: &str,
        workspace: Option<String>,
        skill_key: &str,
        package: &SkillPackage,
        mut target_paths: Vec<String>,
    ) -> SkillRecord {
        target_paths.sort();
        SkillRecord {
            id: skill_entry_id(scope, workspace.as_deref(), skill_key),
            name: package.name.clone(),
            scope: scope.to_string(),
            workspace,
            canonical_source_path: package.canonical_path.display().to_string(),
            target_paths,
            exists: path_exists_or_symlink(&package.canonical_path),
            is_symlink_canonical: is_symlink(&package.canonical_path),
            package_type: package.package_type.clone(),
            skill_key: skill_key.to_string(),
            symlink_target: package.canonical_path.display().to_string(),
            status: SkillLifecycleStatus::Active,
            archived_at: None,
            archived_bundle_path: None,
            archived_original_scope: None,
            archived_original_workspace: None,
        }
    }

    fn create_subagent_entry(
        &self,
        scope: &str,
        workspace: Option<String>,
        subagent_key: &str,
        package: &SubagentPackage,
        mut target_paths: Vec<String>,
    ) -> SubagentRecord {
        target_paths.sort();
        SubagentRecord {
            id: skill_entry_id(scope, workspace.as_deref(), subagent_key),
            name: package.name.clone(),
            description: package.description.clone(),
            scope: scope.to_string(),
            workspace,
            canonical_source_path: package.canonical_path.display().to_string(),
            target_paths,
            exists: path_exists_or_symlink(&package.canonical_path),
            is_symlink_canonical: is_symlink(&package.canonical_path),
            package_type: package.package_type.clone(),
            subagent_key: subagent_key.to_string(),
            symlink_target: package.canonical_path.display().to_string(),
            model: package.model.clone(),
            tools: package.tools.clone(),
            codex_tools_ignored: !package.tools.is_empty(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn make_state(
        &self,
        status: SyncHealthStatus,
        mut entries: Vec<SkillRecord>,
        mut subagent_entries: Vec<SubagentRecord>,
        mcp_outcome: McpSyncOutcome,
        skill_conflict_count: usize,
        subagent_conflict_count: usize,
        started: chrono::DateTime<Utc>,
        finished: chrono::DateTime<Utc>,
        error: Option<String>,
    ) -> SyncState {
        entries.sort_by(sort_entries);
        subagent_entries.sort_by(sort_subagent_entries);
        let top_skill_ids = entries.iter().take(6).map(|item| item.id.clone()).collect();
        let top_subagent_ids = subagent_entries
            .iter()
            .take(6)
            .map(|item| item.id.clone())
            .collect();
        let mcp_warning_count = mcp_outcome.warnings.len();

        SyncState {
            version: 2,
            generated_at: iso8601(finished),
            sync: SyncMetadata {
                status,
                last_started_at: Some(iso8601(started)),
                last_finished_at: Some(iso8601(finished)),
                duration_ms: Some(
                    (finished.timestamp_millis() - started.timestamp_millis()) as u64,
                ),
                error,
                warnings: mcp_outcome.warnings,
            },
            summary: SyncSummary {
                global_count: entries
                    .iter()
                    .filter(|entry| {
                        entry.scope == "global" && entry.status == SkillLifecycleStatus::Active
                    })
                    .count(),
                project_count: entries
                    .iter()
                    .filter(|entry| {
                        entry.scope == "project" && entry.status == SkillLifecycleStatus::Active
                    })
                    .count(),
                conflict_count: skill_conflict_count,
                mcp_count: mcp_outcome.records.len(),
                mcp_warning_count,
            },
            subagent_summary: SyncSummary {
                global_count: subagent_entries
                    .iter()
                    .filter(|entry| entry.scope == "global")
                    .count(),
                project_count: subagent_entries
                    .iter()
                    .filter(|entry| entry.scope == "project")
                    .count(),
                conflict_count: subagent_conflict_count,
                mcp_count: 0,
                mcp_warning_count: 0,
            },
            skills: entries,
            subagents: subagent_entries,
            mcp_servers: mcp_outcome.records,
            top_skills: top_skill_ids,
            top_subagents: top_subagent_ids,
        }
    }

    fn make_failed_state(
        &self,
        previous: SyncState,
        started: chrono::DateTime<Utc>,
        finished: chrono::DateTime<Utc>,
        error: String,
        skill_conflict_count: usize,
        subagent_conflict_count: usize,
    ) -> SyncState {
        SyncState {
            version: 2,
            generated_at: iso8601(finished),
            sync: SyncMetadata {
                status: SyncHealthStatus::Failed,
                last_started_at: Some(iso8601(started)),
                last_finished_at: Some(iso8601(finished)),
                duration_ms: Some(
                    (finished.timestamp_millis() - started.timestamp_millis()) as u64,
                ),
                error: Some(error),
                warnings: previous.sync.warnings,
            },
            summary: SyncSummary {
                global_count: previous.summary.global_count,
                project_count: previous.summary.project_count,
                conflict_count: skill_conflict_count,
                mcp_count: previous.summary.mcp_count,
                mcp_warning_count: previous.summary.mcp_warning_count,
            },
            subagent_summary: SyncSummary {
                global_count: previous.subagent_summary.global_count,
                project_count: previous.subagent_summary.project_count,
                conflict_count: subagent_conflict_count,
                mcp_count: previous.subagent_summary.mcp_count,
                mcp_warning_count: previous.subagent_summary.mcp_warning_count,
            },
            skills: previous.skills,
            subagents: previous.subagents,
            mcp_servers: previous.mcp_servers,
            top_skills: previous.top_skills,
            top_subagents: previous.top_subagents,
        }
    }

    fn discover_global_packages(&self) -> Vec<SkillPackage> {
        let mut result = Vec::new();
        result.extend(self.discover_dir_packages(&self.claude_skills_root()));
        result.extend(self.discover_dir_packages(&self.agents_skills_root()));
        result.extend(self.discover_dir_packages(&self.codex_skills_root()));
        result
    }

    fn discover_project_packages(&self, workspace: &Path) -> Vec<SkillPackage> {
        let mut result = Vec::new();
        result.extend(self.discover_dir_packages(&workspace.join(".claude").join("skills")));
        result.extend(self.discover_dir_packages(&workspace.join(".agents").join("skills")));
        result.extend(self.discover_dir_packages(&workspace.join(".codex").join("skills")));
        result
    }

    fn discover_global_subagents(&self) -> Vec<SubagentPackage> {
        let mut result = Vec::new();
        result.extend(self.discover_subagent_files(&self.shared_subagents_root()));
        result.extend(self.discover_subagent_files(&self.claude_agents_root()));
        result.extend(self.discover_subagent_files(&self.cursor_agents_root()));
        result
    }

    fn discover_project_subagents(&self, workspace: &Path) -> Vec<SubagentPackage> {
        let mut result = Vec::new();
        result.extend(self.discover_subagent_files(&workspace.join(".agents").join("subagents")));
        result.extend(self.discover_subagent_files(&workspace.join(".claude").join("agents")));
        result.extend(self.discover_subagent_files(&workspace.join(".cursor").join("agents")));
        result
    }

    fn discover_dir_packages(&self, root: &Path) -> Vec<SkillPackage> {
        if !root.exists() {
            return Vec::new();
        }

        let mut seen: HashSet<String> = HashSet::new();
        let mut packages = Vec::new();

        for entry in WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            if entry.file_name() != "SKILL.md" {
                continue;
            }
            let path = entry.path().to_path_buf();
            let Some(parent) = path.parent() else {
                continue;
            };
            let Ok(relative_parent) = parent.strip_prefix(root) else {
                continue;
            };

            let skill_key = relative_parent
                .components()
                .map(|segment| segment.as_os_str().to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join("/")
                .trim_matches('/')
                .to_string();

            if skill_key.is_empty()
                || self.is_protected_skill_key(&skill_key)
                || seen.contains(&skill_key)
            {
                continue;
            }
            seen.insert(skill_key.clone());

            let package_hash = match hash_directory(parent) {
                Some(hash) => hash,
                None => continue,
            };

            let name = parent
                .file_name()
                .and_then(OsStr::to_str)
                .unwrap_or(&skill_key)
                .to_string();

            packages.push(SkillPackage {
                source_root: root.to_path_buf(),
                skill_key,
                name,
                canonical_path: parent.to_path_buf(),
                package_type: String::from("dir"),
                package_hash,
            });
        }

        packages
    }

    fn discover_subagent_files(&self, root: &Path) -> Vec<SubagentPackage> {
        if !root.exists() {
            return Vec::new();
        }
        let mut seen: HashSet<String> = HashSet::new();
        let mut packages = Vec::new();
        for entry in WalkDir::new(root)
            .follow_links(false)
            .max_depth(1)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path().to_path_buf();
            if path.extension().and_then(OsStr::to_str) != Some("md") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(OsStr::to_str) else {
                continue;
            };
            let key = stem.trim().to_string();
            if key.is_empty() || self.is_protected_skill_key(&key) || seen.contains(&key) {
                continue;
            }
            let Ok(raw) = fs::read_to_string(&path) else {
                continue;
            };
            let Some(parsed) = parse_subagent_markdown(&raw) else {
                continue;
            };
            if parsed.name != key {
                continue;
            }
            if seen.insert(key.clone()) {
                let package_hash = sha256_bytes(raw.as_bytes());
                packages.push(SubagentPackage {
                    source_root: root.to_path_buf(),
                    subagent_key: key.clone(),
                    name: parsed.name,
                    description: parsed.description,
                    model: parsed.model,
                    tools: parsed.tools,
                    body: parsed.body,
                    canonical_path: path,
                    package_type: String::from("file"),
                    package_hash,
                });
            }
        }
        packages
    }

    fn resolve_scope_candidates(
        &self,
        packages: &[SkillPackage],
        scope: &str,
        workspace: Option<&Path>,
    ) -> (BTreeMap<String, SkillPackage>, Vec<SyncConflict>) {
        let mut by_key: HashMap<String, Vec<SkillPackage>> = HashMap::new();
        for package in packages {
            by_key
                .entry(package.skill_key.clone())
                .or_default()
                .push(package.clone());
        }

        let mut canonical = BTreeMap::new();
        let mut conflicts = Vec::new();

        for (skill_key, candidates) in by_key {
            let hashes: HashSet<String> = candidates
                .iter()
                .map(|item| item.package_hash.clone())
                .collect();
            if hashes.len() > 1 {
                conflicts.push(SyncConflict {
                    kind: SyncConflictKind::Skill,
                    scope: scope.to_string(),
                    workspace: workspace.map(|item| item.display().to_string()),
                    skill_key,
                });
                continue;
            }

            let selected = candidates.into_iter().min_by(|lhs, rhs| {
                let lp = self.source_priority(scope, lhs, workspace);
                let rp = self.source_priority(scope, rhs, workspace);
                lp.cmp(&rp)
                    .then_with(|| lhs.source_root.cmp(&rhs.source_root))
                    .then_with(|| lhs.canonical_path.cmp(&rhs.canonical_path))
            });

            if let Some(package) = selected {
                canonical.insert(package.skill_key.clone(), package);
            }
        }

        (canonical, conflicts)
    }

    fn resolve_subagent_scope_candidates(
        &self,
        packages: &[SubagentPackage],
        scope: &str,
        workspace: Option<&Path>,
    ) -> (BTreeMap<String, SubagentPackage>, Vec<SyncConflict>) {
        let mut by_key: HashMap<String, Vec<SubagentPackage>> = HashMap::new();
        for package in packages {
            by_key
                .entry(package.subagent_key.clone())
                .or_default()
                .push(package.clone());
        }

        let mut canonical = BTreeMap::new();
        let mut conflicts = Vec::new();
        for (subagent_key, candidates) in by_key {
            let hashes: HashSet<String> = candidates
                .iter()
                .map(|item| item.package_hash.clone())
                .collect();
            if hashes.len() > 1 {
                conflicts.push(SyncConflict {
                    kind: SyncConflictKind::Subagent,
                    scope: scope.to_string(),
                    workspace: workspace.map(|item| item.display().to_string()),
                    skill_key: subagent_key,
                });
                continue;
            }

            let selected = candidates.into_iter().min_by(|lhs, rhs| {
                let lp = self.subagent_source_priority(scope, lhs, workspace);
                let rp = self.subagent_source_priority(scope, rhs, workspace);
                lp.cmp(&rp)
                    .then_with(|| lhs.source_root.cmp(&rhs.source_root))
                    .then_with(|| lhs.canonical_path.cmp(&rhs.canonical_path))
            });
            if let Some(package) = selected {
                canonical.insert(package.subagent_key.clone(), package);
            }
        }
        (canonical, conflicts)
    }

    fn source_priority(
        &self,
        scope: &str,
        package: &SkillPackage,
        workspace: Option<&Path>,
    ) -> usize {
        if scope == "global" {
            let roots = [
                self.claude_skills_root(),
                self.agents_skills_root(),
                self.codex_skills_root(),
            ];
            for (idx, root) in roots.iter().enumerate() {
                if standardized_path(root) == standardized_path(&package.source_root) {
                    return idx;
                }
            }
            return usize::MAX;
        }

        let Some(workspace) = workspace else {
            return usize::MAX;
        };
        let claude = workspace.join(".claude").join("skills");
        let agents = workspace.join(".agents").join("skills");
        let codex = workspace.join(".codex").join("skills");

        if standardized_path(&package.source_root) == standardized_path(&claude) {
            return 0;
        }
        if standardized_path(&package.source_root) == standardized_path(&agents) {
            return 1;
        }
        if standardized_path(&package.source_root) == standardized_path(&codex) {
            return 2;
        }

        usize::MAX
    }

    fn subagent_source_priority(
        &self,
        scope: &str,
        package: &SubagentPackage,
        workspace: Option<&Path>,
    ) -> usize {
        if scope == "global" {
            let roots = [
                self.shared_subagents_root(),
                self.claude_agents_root(),
                self.cursor_agents_root(),
            ];
            for (idx, root) in roots.iter().enumerate() {
                if standardized_path(root) == standardized_path(&package.source_root) {
                    return idx;
                }
            }
            return usize::MAX;
        }
        let Some(workspace) = workspace else {
            return usize::MAX;
        };
        let shared = workspace.join(".agents").join("subagents");
        let claude = workspace.join(".claude").join("agents");
        let cursor = workspace.join(".cursor").join("agents");
        if standardized_path(&package.source_root) == standardized_path(&shared) {
            return 0;
        }
        if standardized_path(&package.source_root) == standardized_path(&claude) {
            return 1;
        }
        if standardized_path(&package.source_root) == standardized_path(&cursor) {
            return 2;
        }
        usize::MAX
    }

    fn migrate_scope_candidates_to_claude(
        &self,
        candidates: &[SkillPackage],
        scope: &str,
        workspace: Option<&Path>,
    ) -> Result<BTreeMap<String, SkillPackage>, SyncEngineError> {
        let mut by_key: HashMap<String, Vec<SkillPackage>> = HashMap::new();
        for candidate in candidates {
            by_key
                .entry(candidate.skill_key.clone())
                .or_default()
                .push(candidate.clone());
        }

        let canonical_root = if scope == "global" {
            self.claude_skills_root()
        } else {
            workspace
                .map(|w| w.join(".claude").join("skills"))
                .ok_or_else(|| {
                    SyncEngineError::Unsupported(String::from(
                        "workspace is required for project migration",
                    ))
                })?
        };

        let mut canonical_by_key = BTreeMap::new();
        for (skill_key, options) in by_key {
            let hashes: HashSet<String> = options
                .iter()
                .map(|item| item.package_hash.clone())
                .collect();
            if hashes.len() > 1 {
                return Err(SyncEngineError::conflicts(vec![SyncConflict {
                    kind: SyncConflictKind::Skill,
                    scope: scope.to_string(),
                    workspace: workspace.map(|item| item.display().to_string()),
                    skill_key,
                }]));
            }

            let desired = canonical_root.join(&skill_key);
            if let Some(parent) = desired.parent() {
                fs::create_dir_all(parent).map_err(|e| SyncEngineError::io(parent, e))?;
            }

            let selected = options.iter().min_by(|lhs, rhs| {
                self.source_priority(scope, lhs, workspace)
                    .cmp(&self.source_priority(scope, rhs, workspace))
                    .then_with(|| lhs.canonical_path.cmp(&rhs.canonical_path))
            });

            let Some(selected) = selected else {
                continue;
            };
            let selected_path = selected.canonical_path.clone();
            let selected_skill_key = selected.skill_key.clone();
            let selected_package_hash = selected.package_hash.clone();

            if standardized_path(&selected_path) != standardized_path(&desired)
                && path_exists_or_symlink(&selected_path)
            {
                if path_exists_or_symlink(&desired) {
                    if is_symlink(&desired) || desired.is_dir() {
                        remove_path(&desired)?;
                    } else {
                        return Err(SyncEngineError::MigrationFailed {
                            skill_key,
                            reason: format!("canonical path occupied: {}", desired.display()),
                        });
                    }
                }

                fs::rename(&selected_path, &desired).map_err(|e| {
                    SyncEngineError::MigrationFailed {
                        skill_key: selected_skill_key.clone(),
                        reason: e.to_string(),
                    }
                })?;
            }

            for option in options {
                if standardized_path(&option.canonical_path) == standardized_path(&desired) {
                    continue;
                }
                if !path_exists_or_symlink(&option.canonical_path) {
                    continue;
                }
                self.create_or_update_symlink(&option.canonical_path, &desired)
                    .map_err(|error| SyncEngineError::MigrationFailed {
                        skill_key: option.skill_key.clone(),
                        reason: error.to_string(),
                    })?;
            }

            canonical_by_key.insert(
                skill_key.clone(),
                SkillPackage {
                    source_root: canonical_root.clone(),
                    skill_key: skill_key.clone(),
                    name: desired
                        .file_name()
                        .and_then(OsStr::to_str)
                        .unwrap_or(&skill_key)
                        .to_string(),
                    canonical_path: desired,
                    package_type: String::from("dir"),
                    package_hash: selected_package_hash,
                },
            );
        }

        Ok(canonical_by_key)
    }

    fn workspace_candidates(&self) -> Vec<PathBuf> {
        let mut candidates: Vec<PathBuf> = Vec::new();

        if let Ok(entries) = fs::read_dir(&self.environment.dev_root) {
            for entry in entries.filter_map(Result::ok) {
                let repo = entry.path();
                if self.has_workspace_sync_roots(&repo) {
                    candidates.push(repo);
                }
            }
        }

        if let Ok(owners) = fs::read_dir(&self.environment.worktrees_root) {
            for owner in owners.filter_map(Result::ok) {
                if let Ok(repos) = fs::read_dir(owner.path()) {
                    for repo in repos.filter_map(Result::ok) {
                        let path = repo.path();
                        if self.has_workspace_sync_roots(&path) {
                            candidates.push(path);
                        }
                    }
                }
            }
        }

        for root in self.custom_workspace_discovery_roots() {
            candidates.extend(self.discover_workspaces(&root, 0, 3));
        }

        let mut unique = HashMap::new();
        for path in candidates {
            unique.insert(standardized_path(&path), path);
        }

        let mut deduped: Vec<PathBuf> = unique.into_values().collect();
        deduped.sort();
        deduped
    }

    fn has_workspace_sync_roots(&self, repo: &Path) -> bool {
        repo.join(".claude").join("skills").exists()
            || repo.join(".agents").join("skills").exists()
            || repo.join(".codex").join("skills").exists()
            || repo.join(".claude").join("agents").exists()
            || repo.join(".cursor").join("agents").exists()
            || repo.join(".agents").join("subagents").exists()
            || repo.join(".mcp.json").exists()
            || repo.join(".codex").join("config.toml").exists()
    }

    fn custom_workspace_discovery_roots(&self) -> Vec<PathBuf> {
        let configured = self
            .preferences_store
            .load_settings()
            .workspace_discovery_roots;
        let mut roots = Vec::new();
        let mut seen = HashSet::new();

        for raw in configured {
            let trimmed = raw.trim();
            if trimmed.is_empty() || !trimmed.starts_with('/') {
                continue;
            }
            let path = PathBuf::from(trimmed);
            let key = standardized_path(&path);
            if seen.insert(key) {
                roots.push(path);
            }
        }

        roots
    }

    fn sanitize_starred_skill_ids(
        &self,
        state: &SyncState,
        settings: &crate::settings::SyncAppSettings,
    ) -> Vec<String> {
        let allowed: HashSet<&str> = state.skills.iter().map(|skill| skill.id.as_str()).collect();
        let mut seen: HashSet<String> = HashSet::new();
        let mut ids = Vec::new();
        let starred = settings
            .ui_state
            .as_ref()
            .map(|ui| ui.starred_skill_ids.as_slice())
            .unwrap_or(&[]);

        for id in starred {
            if !allowed.contains(id.as_str()) {
                continue;
            }
            if seen.insert(id.clone()) {
                ids.push(id.clone());
            }
        }
        ids
    }

    fn save_starred_skill_ids(
        &self,
        mut settings: crate::settings::SyncAppSettings,
        starred_skill_ids: Vec<String>,
    ) -> Result<(), SyncEngineError> {
        let ui_state = settings.ui_state.get_or_insert_with(AppUiState::default);
        ui_state.starred_skill_ids = starred_skill_ids;
        self.preferences_store.save_settings(&settings)
    }

    fn migrate_starred_skill_id(
        &self,
        old_id: &str,
        new_id: &str,
        state: &SyncState,
    ) -> Result<(), SyncEngineError> {
        let settings = self.preferences_store.load_settings();
        let raw_starred = settings
            .ui_state
            .as_ref()
            .map(|ui| ui.starred_skill_ids.clone())
            .unwrap_or_default();
        let had_old = raw_starred.iter().any(|id| id == old_id);
        if !had_old {
            return Ok(());
        }

        let allowed: HashSet<&str> = state.skills.iter().map(|skill| skill.id.as_str()).collect();
        let mut seen: HashSet<String> = HashSet::new();
        let mut starred = Vec::new();

        for id in raw_starred {
            let mapped = if id == old_id { new_id } else { id.as_str() };
            if !allowed.contains(mapped) {
                continue;
            }
            if seen.insert(mapped.to_string()) {
                starred.push(mapped.to_string());
            }
        }

        self.save_starred_skill_ids(settings, starred)
    }

    fn discover_workspaces(&self, root: &Path, depth: usize, max_depth: usize) -> Vec<PathBuf> {
        if !root.exists() {
            return Vec::new();
        }

        let mut result = Vec::new();
        if self.has_workspace_sync_roots(root) {
            result.push(root.to_path_buf());
        }

        if depth >= max_depth {
            return result;
        }

        let Ok(children) = fs::read_dir(root) else {
            return result;
        };

        for child in children.filter_map(Result::ok) {
            let path = child.path();
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                continue;
            };
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                continue;
            }
            result.extend(self.discover_workspaces(&path, depth + 1, max_depth));
        }

        result
    }

    fn create_or_update_symlink(
        &self,
        target: &Path,
        destination: &Path,
    ) -> Result<(), SyncEngineError> {
        if path_exists_or_symlink(target) {
            if is_symlink(target) {
                if let Ok(existing_destination) = fs::read_link(target) {
                    let existing_absolute = if existing_destination.is_absolute() {
                        existing_destination
                    } else {
                        target
                            .parent()
                            .unwrap_or_else(|| Path::new("/"))
                            .join(existing_destination)
                    };
                    if standardized_path(&existing_absolute) == standardized_path(destination) {
                        return Ok(());
                    }
                }
            }
            remove_path(target)?;
        }

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| SyncEngineError::io(parent, e))?;
        }

        create_symlink(destination, target)
    }

    fn ensure_directories(&self) -> Result<(), SyncEngineError> {
        let dirs = vec![
            self.claude_skills_root(),
            self.agents_skills_root(),
            self.codex_skills_root(),
            self.shared_subagents_root(),
            self.claude_agents_root(),
            self.cursor_agents_root(),
            self.archives_root(),
            self.runtime_skills_root(),
            self.runtime_prompts_root(),
            self.environment.runtime_directory.clone(),
        ];

        for dir in dirs {
            fs::create_dir_all(&dir).map_err(|e| SyncEngineError::io(&dir, e))?;
        }

        Ok(())
    }

    fn cleanup_stale_links(
        &self,
        old_managed_links: &HashSet<String>,
        new_managed_links: &HashSet<String>,
    ) {
        for stale in old_managed_links.difference(new_managed_links) {
            let path = PathBuf::from(stale);
            if is_symlink(&path) {
                let _ = fs::remove_file(&path);
            }
        }
    }

    fn load_managed_links_manifest(&self) -> HashSet<String> {
        #[derive(Debug, Deserialize)]
        struct Manifest {
            #[serde(default, rename = "managed_links")]
            managed_links: Vec<String>,
        }

        let manifest = self
            .environment
            .runtime_directory
            .join(".skill-sync-manifest.json");
        let Ok(data) = fs::read(&manifest) else {
            return HashSet::new();
        };
        let Ok(payload) = serde_json::from_slice::<Manifest>(&data) else {
            return HashSet::new();
        };
        payload.managed_links.into_iter().collect()
    }

    fn load_subagent_managed_links_manifest(&self) -> HashSet<String> {
        #[derive(Debug, Deserialize)]
        struct Manifest {
            #[serde(default, rename = "managed_links")]
            managed_links: Vec<String>,
        }

        let manifest = self
            .environment
            .runtime_directory
            .join(".subagent-sync-manifest.json");
        let Ok(data) = fs::read(&manifest) else {
            return HashSet::new();
        };
        let Ok(payload) = serde_json::from_slice::<Manifest>(&data) else {
            return HashSet::new();
        };
        payload.managed_links.into_iter().collect()
    }

    fn save_managed_links_manifest(
        &self,
        managed_links: &HashSet<String>,
    ) -> Result<(), SyncEngineError> {
        #[derive(Debug, Serialize)]
        struct Manifest<'a> {
            version: u32,
            #[serde(rename = "generated_at")]
            generated_at: &'a str,
            #[serde(rename = "managed_links")]
            managed_links: Vec<String>,
        }

        let path = self
            .environment
            .runtime_directory
            .join(".skill-sync-manifest.json");
        let generated_at = iso8601_now();
        let payload = Manifest {
            version: 1,
            generated_at: &generated_at,
            managed_links: {
                let mut links: Vec<String> = managed_links.iter().cloned().collect();
                links.sort();
                links
            },
        };

        let mut data = serde_json::to_vec_pretty(&payload)?;
        data.push(b'\n');
        fs::write(&path, data).map_err(|e| SyncEngineError::io(&path, e))
    }

    fn save_subagent_managed_links_manifest(
        &self,
        managed_links: &HashSet<String>,
    ) -> Result<(), SyncEngineError> {
        #[derive(Debug, Serialize)]
        struct Manifest<'a> {
            version: u32,
            #[serde(rename = "generated_at")]
            generated_at: &'a str,
            #[serde(rename = "managed_links")]
            managed_links: Vec<String>,
        }

        let path = self
            .environment
            .runtime_directory
            .join(".subagent-sync-manifest.json");
        let generated_at = iso8601_now();
        let payload = Manifest {
            version: 1,
            generated_at: &generated_at,
            managed_links: {
                let mut links: Vec<String> = managed_links.iter().cloned().collect();
                links.sort();
                links
            },
        };
        let mut data = serde_json::to_vec_pretty(&payload)?;
        data.push(b'\n');
        fs::write(&path, data).map_err(|e| SyncEngineError::io(&path, e))
    }

    fn move_to_trash(&self, path: &Path) -> Result<PathBuf, SyncEngineError> {
        let trash = self.environment.home_directory.join(".Trash");
        fs::create_dir_all(&trash).map_err(|e| SyncEngineError::io(&trash, e))?;

        let base_name = path
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or("skill")
            .to_string();

        let mut destination = trash.join(&base_name);
        let mut index: usize = 1;
        while path_exists_or_symlink(&destination) {
            destination = trash.join(format!("{base_name}.{index}"));
            index += 1;
        }

        fs::rename(path, &destination).map_err(|e| SyncEngineError::io(path, e))?;
        Ok(destination)
    }

    fn is_protected_path(&self, path: &Path) -> bool {
        path.components().any(|component| {
            self.protected_segments
                .contains(component.as_os_str().to_string_lossy().as_ref())
        })
    }

    fn is_protected_skill_key(&self, key: &str) -> bool {
        key.split('/')
            .any(|segment| self.protected_segments.contains(segment))
    }

    fn is_relative_to(&self, path: &Path, base: &Path) -> bool {
        let base = standardized_path(base);
        let candidate = standardized_path(path);
        candidate == base || candidate.starts_with(&format!("{base}/"))
    }

    fn write_archived_manifest(
        &self,
        manifest: &ArchivedSkillManifest,
        bundle: &Path,
    ) -> Result<(), SyncEngineError> {
        let path = bundle.join("manifest.json");
        let mut data = serde_json::to_vec_pretty(manifest)?;
        data.push(b'\n');
        fs::write(&path, data).map_err(|_| SyncEngineError::ArchiveManifestWriteFailed)
    }

    fn read_archived_manifest(
        &self,
        bundle: &Path,
    ) -> Result<ArchivedSkillManifest, SyncEngineError> {
        let path = bundle.join("manifest.json");
        if !path.exists() {
            return Err(SyncEngineError::RestoreManifestMissing);
        }

        let data = fs::read(&path).map_err(|_| SyncEngineError::RestoreManifestMissing)?;
        serde_json::from_slice(&data).map_err(|_| SyncEngineError::RestoreManifestMissing)
    }

    fn load_archived_entries(&self) -> Vec<SkillRecord> {
        let archives_root = self.archives_root();
        let Ok(entries) = fs::read_dir(&archives_root) else {
            return Vec::new();
        };

        let mut result = Vec::new();
        for entry in entries.filter_map(Result::ok) {
            let bundle = entry.path();
            let Ok(metadata) = fs::symlink_metadata(&bundle) else {
                continue;
            };
            if !metadata.is_dir() {
                continue;
            }

            let Ok(manifest) = self.read_archived_manifest(&bundle) else {
                continue;
            };
            let source = bundle.join("source");
            let exists = path_exists_or_symlink(&source);
            let scope = if manifest.original_scope.trim().is_empty() {
                String::from("global")
            } else {
                manifest.original_scope.clone()
            };

            result.push(SkillRecord {
                id: skill_entry_id(
                    "archived",
                    Some(bundle.to_string_lossy().as_ref()),
                    &manifest.skill_key,
                ),
                name: manifest.name.clone(),
                scope,
                workspace: manifest.original_workspace.clone(),
                canonical_source_path: source.display().to_string(),
                target_paths: manifest.moved_links.clone(),
                exists,
                is_symlink_canonical: is_symlink(&source),
                package_type: String::from("dir"),
                skill_key: manifest.skill_key.clone(),
                symlink_target: source.display().to_string(),
                status: SkillLifecycleStatus::Archived,
                archived_at: Some(manifest.archived_at.clone()),
                archived_bundle_path: Some(bundle.display().to_string()),
                archived_original_scope: Some(manifest.original_scope.clone()),
                archived_original_workspace: manifest.original_workspace.clone(),
            });
        }

        result
    }

    fn unique_archived_link_path(
        &self,
        base_name: &str,
        links_root: &Path,
        used: &mut HashSet<String>,
    ) -> PathBuf {
        let trimmed = base_name.trim();
        let root = if trimmed.is_empty() { "link" } else { trimmed };
        let mut candidate = root.to_string();
        let mut index = 1;

        while used.contains(&candidate) || links_root.join(&candidate).exists() {
            candidate = format!("{root}-{index}");
            index += 1;
        }

        used.insert(candidate.clone());
        links_root.join(candidate)
    }

    fn make_archive_bundle_name(&self, skill_key: &str) -> String {
        let safe_key = skill_key.replace('/', "--");
        let compact_time = iso8601_now().replace([':', '-'], "");
        let short = Uuid::new_v4().simple().to_string();
        format!("{compact_time}-{safe_key}-{}", &short[..8])
    }

    fn allowed_delete_roots(&self) -> Vec<PathBuf> {
        let mut roots = vec![
            self.claude_skills_root(),
            self.agents_skills_root(),
            self.codex_skills_root(),
            self.archives_root(),
        ];
        for workspace in self.workspace_candidates() {
            roots.extend(self.project_targets(&workspace));
        }
        roots
    }

    fn allowed_project_roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        for workspace in self.workspace_candidates() {
            roots.extend(self.project_targets(&workspace));
        }
        roots
    }

    fn claude_skills_root(&self) -> PathBuf {
        self.environment
            .home_directory
            .join(".claude")
            .join("skills")
    }

    fn agents_skills_root(&self) -> PathBuf {
        self.environment
            .home_directory
            .join(".agents")
            .join("skills")
    }

    fn codex_skills_root(&self) -> PathBuf {
        self.environment
            .home_directory
            .join(".codex")
            .join("skills")
    }

    fn shared_subagents_root(&self) -> PathBuf {
        self.environment
            .home_directory
            .join(".agents")
            .join("subagents")
    }

    fn claude_agents_root(&self) -> PathBuf {
        self.environment
            .home_directory
            .join(".claude")
            .join("agents")
    }

    fn cursor_agents_root(&self) -> PathBuf {
        self.environment
            .home_directory
            .join(".cursor")
            .join("agents")
    }

    fn archives_root(&self) -> PathBuf {
        self.environment.runtime_directory.join("archives")
    }

    fn runtime_skills_root(&self) -> PathBuf {
        self.environment
            .home_directory
            .join(".config")
            .join("ai-agents")
            .join("skillssync")
    }

    fn runtime_prompts_root(&self) -> PathBuf {
        self.environment
            .home_directory
            .join(".config")
            .join("ai-agents")
            .join("prompts")
    }

    fn preferred_global_destination(&self, skill_key: &str) -> PathBuf {
        self.claude_skills_root().join(skill_key)
    }

    fn global_targets(&self) -> Vec<PathBuf> {
        vec![
            self.claude_skills_root(),
            self.agents_skills_root(),
            self.codex_skills_root(),
        ]
    }

    fn global_subagent_targets(&self) -> Vec<PathBuf> {
        vec![
            self.shared_subagents_root(),
            self.claude_agents_root(),
            self.cursor_agents_root(),
        ]
    }

    fn project_targets(&self, workspace: &Path) -> Vec<PathBuf> {
        vec![
            workspace.join(".claude").join("skills"),
            workspace.join(".agents").join("skills"),
            workspace.join(".codex").join("skills"),
        ]
    }

    fn project_subagent_targets(&self, workspace: &Path) -> Vec<PathBuf> {
        vec![
            workspace.join(".agents").join("subagents"),
            workspace.join(".claude").join("agents"),
            workspace.join(".cursor").join("agents"),
        ]
    }
}

fn create_symlink(destination: &Path, target: &Path) -> Result<(), SyncEngineError> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(destination, target).map_err(|e| SyncEngineError::io(target, e))
    }
    #[cfg(windows)]
    {
        if destination.is_dir() {
            std::os::windows::fs::symlink_dir(destination, target)
                .map_err(|e| SyncEngineError::io(target, e))
        } else {
            std::os::windows::fs::symlink_file(destination, target)
                .map_err(|e| SyncEngineError::io(target, e))
        }
    }
}

fn remove_path(path: &Path) -> Result<(), SyncEngineError> {
    let metadata = fs::symlink_metadata(path).map_err(|e| SyncEngineError::io(path, e))?;
    if metadata.file_type().is_symlink() || metadata.is_file() {
        fs::remove_file(path).map_err(|e| SyncEngineError::io(path, e))
    } else {
        fs::remove_dir_all(path).map_err(|e| SyncEngineError::io(path, e))
    }
}

fn path_exists_or_symlink(path: &Path) -> bool {
    path.exists()
        || fs::symlink_metadata(path)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
}

fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}

fn standardized_path(path: &Path) -> String {
    fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string()
}

fn iso8601(value: chrono::DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn iso8601_now() -> String {
    iso8601(Utc::now())
}

fn skill_entry_id(scope: &str, workspace: Option<&str>, skill_key: &str) -> String {
    let workspace_value = workspace.unwrap_or("global");
    let value = format!("{scope}|{workspace_value}|{skill_key}");
    let digest = Sha1::digest(value.as_bytes());
    let hex = hex_encode(&digest);
    format!("skill-{}", &hex[..12])
}

fn sort_entries(lhs: &SkillRecord, rhs: &SkillRecord) -> std::cmp::Ordering {
    lhs.status
        .cmp(&rhs.status)
        .then_with(|| lhs.scope.cmp(&rhs.scope))
        .then_with(|| {
            lhs.name
                .to_ascii_lowercase()
                .cmp(&rhs.name.to_ascii_lowercase())
        })
        .then_with(|| lhs.workspace.cmp(&rhs.workspace))
}

fn sort_subagent_entries(lhs: &SubagentRecord, rhs: &SubagentRecord) -> std::cmp::Ordering {
    lhs.scope
        .cmp(&rhs.scope)
        .then_with(|| {
            lhs.name
                .to_ascii_lowercase()
                .cmp(&rhs.name.to_ascii_lowercase())
        })
        .then_with(|| lhs.workspace.cmp(&rhs.workspace))
}

fn normalized_skill_key(title: &str) -> String {
    let trimmed = title.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut result = String::new();
    let mut previous_dash = false;
    for ch in trimmed.chars() {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            result.push(ch);
            previous_dash = false;
        } else if !previous_dash {
            result.push('-');
            previous_dash = true;
        }
    }

    result.trim_matches('-').to_string()
}

fn update_skill_title(path: &Path, new_title: &str) -> Result<(), SyncEngineError> {
    let contents = fs::read_to_string(path).map_err(|e| SyncEngineError::io(path, e))?;
    let updated = updated_skill_contents(&contents, new_title);
    fs::write(path, updated).map_err(|e| SyncEngineError::io(path, e))
}

fn updated_skill_contents(original: &str, title: &str) -> String {
    let normalized = original.replace("\r\n", "\n");
    if let Some(without_frontmatter_start) = normalized.strip_prefix("---\n") {
        if let Some(fm_end_idx) = without_frontmatter_start.find("\n---") {
            let fm_raw = &without_frontmatter_start[..fm_end_idx];
            let mut lines: Vec<String> = fm_raw.lines().map(|line| line.to_string()).collect();
            let mut replaced = false;
            for line in &mut lines {
                if let Some((key, _value)) = line.split_once(':') {
                    if key.trim().eq_ignore_ascii_case("title") {
                        *line = format!("title: {title}");
                        replaced = true;
                        break;
                    }
                }
            }
            if !replaced {
                lines.push(format!("title: {title}"));
            }
            let suffix = &without_frontmatter_start[(fm_end_idx + 4)..];
            return format!("---\n{}\n---{suffix}", lines.join("\n"));
        }
    }

    format!("---\ntitle: {title}\n---\n\n{normalized}")
}

fn hash_directory(directory: &Path) -> Option<String> {
    if !directory.exists() {
        return None;
    }

    let mut files = Vec::new();
    for entry in WalkDir::new(directory)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        let file_type = entry.file_type();
        if file_type.is_file() || file_type.is_symlink() {
            files.push(path.to_path_buf());
        }
    }
    files.sort();

    let mut digest = Sha256::new();
    if files.is_empty() {
        digest.update(b"<empty>");
    } else {
        for file in files {
            let relative = file.strip_prefix(directory).ok()?.to_string_lossy();
            digest.update(relative.as_bytes());
            digest.update([0u8]);

            if is_symlink(&file) {
                match fs::read_link(&file) {
                    Ok(target) => {
                        let resolved = if target.is_absolute() {
                            target
                        } else {
                            file.parent().unwrap_or(directory).join(target)
                        };
                        if resolved.exists() && resolved.is_file() {
                            let Ok(bytes) = fs::read(&resolved) else {
                                return None;
                            };
                            digest.update(bytes);
                        } else {
                            digest.update(b"<broken-symlink>");
                        }
                    }
                    Err(_) => digest.update(b"<broken-symlink>"),
                }
            } else {
                let Ok(mut handle) = fs::File::open(&file) else {
                    return None;
                };
                let mut buf = Vec::new();
                if handle.read_to_end(&mut buf).is_err() {
                    return None;
                }
                digest.update(buf);
            }
            digest.update([0u8]);
        }
    }

    Some(hex_encode(&digest.finalize()))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    hex_encode(&digest.finalize())
}

#[derive(Debug)]
struct ParsedSubagent {
    name: String,
    description: String,
    model: Option<String>,
    tools: Vec<String>,
    body: String,
}

fn parse_subagent_markdown(raw: &str) -> Option<ParsedSubagent> {
    let normalized = raw.replace("\r\n", "\n");
    let rest = normalized.strip_prefix("---\n")?;
    let fm_end = rest.find("\n---")?;
    let frontmatter = &rest[..fm_end];
    let body = rest[(fm_end + 4)..].trim().to_string();
    let mut name = None;
    let mut description = None;
    let mut model = None;
    let mut tools: Vec<String> = Vec::new();

    for line in frontmatter.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();
        match key {
            "name" => name = Some(value),
            "description" => description = Some(value),
            "model" => model = Some(value),
            "tools" => {
                let value = value.trim();
                if value.starts_with('[') && value.ends_with(']') {
                    let inner = &value[1..value.len().saturating_sub(1)];
                    for item in inner.split(',') {
                        let token = item.trim().trim_matches('"').trim_matches('\'');
                        if !token.is_empty() {
                            tools.push(token.to_string());
                        }
                    }
                } else if !value.is_empty() {
                    for item in value.split(',') {
                        let token = item.trim().trim_matches('"').trim_matches('\'');
                        if !token.is_empty() {
                            tools.push(token.to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let name = name?;
    let description = description?;
    if !is_valid_subagent_key(&name) {
        return None;
    }
    Some(ParsedSubagent {
        name,
        description,
        model: model.filter(|item| !item.trim().is_empty()),
        tools,
        body,
    })
}

fn is_valid_subagent_key(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return false;
    }
    chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{:02x}", byte);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{normalized_skill_key, updated_skill_contents};

    #[test]
    fn normalized_skill_key_removes_noise() {
        assert_eq!(
            normalized_skill_key("  My Skill ++ Name  "),
            "my-skill-name"
        );
        assert_eq!(normalized_skill_key("___"), "");
    }

    #[test]
    fn updated_skill_contents_replaces_frontmatter_title() {
        let raw = "---\ntitle: Old\n---\n\nBody";
        let updated = updated_skill_contents(raw, "New");
        assert!(updated.contains("title: New"));
    }
}
