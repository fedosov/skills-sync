use crate::error::{render_json_pretty, write_json_pretty, SyncEngineError};
use crate::managed_block::{strip_managed_blocks, upsert_managed_block};
use crate::models::{
    CatalogMutationAction, McpEnabledByAgent, McpServerRecord, McpTransport, SkillLifecycleStatus,
};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

const CENTRAL_BEGIN: &str = "# agent-sync:mcp:begin";
const CENTRAL_END: &str = "# agent-sync:mcp:end";
const CODEX_BEGIN: &str = "# agent-sync:mcp:codex:begin";
const CODEX_END: &str = "# agent-sync:mcp:codex:end";
const LEGACY_CENTRAL_BEGIN: &str = "# skills-sync:mcp:begin";
const LEGACY_CENTRAL_END: &str = "# skills-sync:mcp:end";
const LEGACY_CODEX_BEGIN: &str = "# skills-sync:mcp:codex:begin";
const LEGACY_CODEX_END: &str = "# skills-sync:mcp:codex:end";
const CENTRAL_MARKER_PAIRS: [(&str, &str); 2] = [
    (CENTRAL_BEGIN, CENTRAL_END),
    (LEGACY_CENTRAL_BEGIN, LEGACY_CENTRAL_END),
];
const CODEX_MARKER_PAIRS: [(&str, &str); 2] = [
    (CODEX_BEGIN, CODEX_END),
    (LEGACY_CODEX_BEGIN, LEGACY_CODEX_END),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum McpAgent {
    Codex,
    Claude,
    Project,
}

impl McpAgent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Project => "project",
        }
    }
}

impl std::str::FromStr for McpAgent {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "codex" => Ok(Self::Codex),
            "claude" => Ok(Self::Claude),
            "project" => Ok(Self::Project),
            other => Err(format!("unsupported agent: {other} (codex|claude|project)")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct McpSyncOutcome {
    pub records: Vec<McpServerRecord>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnmanagedClaudeMcpCandidate {
    pub server_key: String,
    pub scope: String,
    pub workspace: Option<String>,
    pub file_path: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnmanagedClaudeMcpFixReport {
    pub apply: bool,
    pub candidates: Vec<UnmanagedClaudeMcpCandidate>,
    pub removed_count: usize,
    pub changed_files: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum McpScope {
    Global,
    Project,
}

impl McpScope {
    fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Project => "project",
        }
    }

    fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "global" => Ok(Self::Global),
            "project" => Ok(Self::Project),
            other => Err(format!("unsupported scope: {other} (global|project)")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct McpDefinition {
    transport: McpTransport,
    command: Option<String>,
    args: Vec<String>,
    url: Option<String>,
    env: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectClaudeTarget {
    WorkspaceMcpJson,
    ClaudeUserProject,
}

impl ProjectClaudeTarget {
    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "workspace_mcp_json" => Some(Self::WorkspaceMcpJson),
            "claude_user_project" => Some(Self::ClaudeUserProject),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::WorkspaceMcpJson => "workspace_mcp_json",
            Self::ClaudeUserProject => "claude_user_project",
        }
    }
}

#[derive(Debug, Clone)]
struct CatalogEntry {
    catalog_id: String,
    server_key: String,
    scope: McpScope,
    workspace: Option<String>,
    definition: McpDefinition,
    enabled_by_agent: McpEnabledByAgent,
    project_claude_target: ProjectClaudeTarget,
    status: SkillLifecycleStatus,
    archived_at: Option<String>,
}

#[derive(Debug, Clone)]
struct CodexCatalogEntry {
    definition: McpDefinition,
    enabled: bool,
}

#[derive(Debug, Clone)]
struct ManagedCodexObserved {
    catalog_id: String,
    server_key: String,
    enabled: bool,
    file_path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SourceKind {
    CodexGlobal,
    ClaudeUserGlobal,
    ClaudeLocalGlobal,
    ClaudeGlobalGlobal,
    ClaudeUserProject,
    ProjectCodex,
    ProjectClaude,
}

#[derive(Debug, Clone)]
struct Discovered {
    source: SourceKind,
    file_path: PathBuf,
    entry: CatalogEntry,
    enabled: bool,
}

#[derive(Debug, Clone)]
struct JsonCatalogEntry {
    locator: String,
    server_key: String,
    definition: McpDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum JsonTargetLocation {
    Root,
    Project { workspace: String },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum ClaudeJsonTargetLocation {
    Root,
    Project { workspace: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum WarningFixAction {
    UnmanagedInCentral {
        server_key: String,
        catalog_id: String,
        file_path: String,
    },
    InlineSecretEnv {
        server_key: String,
        env_key: String,
    },
    InlineSecretArgument {
        server_key: String,
        redacted_argument: String,
    },
    SkippedManagedCodex {
        server_key: String,
        file_path: String,
    },
    SkippedMissingProjectTarget {
        file_path: String,
    },
}

#[derive(Debug, Clone)]
struct JsonWritePlan {
    path: PathBuf,
    location: JsonTargetLocation,
    create_when_missing: bool,
    entries: Vec<JsonCatalogEntry>,
}

#[derive(Debug, Clone)]
struct BrokenUnmanagedClaudeCandidate {
    output: UnmanagedClaudeMcpCandidate,
    path: PathBuf,
    location: ClaudeJsonTargetLocation,
    server_key: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct McpManifest {
    version: u32,
    #[serde(rename = "generated_at")]
    generated_at: String,
    #[serde(default)]
    targets: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    codex_enabled: BTreeMap<String, bool>,
}

pub struct McpRegistry {
    home_directory: PathBuf,
    runtime_directory: PathBuf,
}

impl McpRegistry {
    pub fn new(home_directory: PathBuf, runtime_directory: PathBuf) -> Self {
        Self {
            home_directory,
            runtime_directory,
        }
    }

    pub fn sync(&self, workspaces: &[PathBuf]) -> Result<McpSyncOutcome, SyncEngineError> {
        let mut warnings = Vec::new();
        let discovered = self.discover_all(workspaces, &mut warnings);
        let previous_manifest = self.load_manifest();

        let mut catalog = self.load_central_catalog(&mut warnings)?;
        if catalog.is_empty() {
            catalog = self.bootstrap_catalog(&discovered, &mut warnings);
        } else {
            let mut stale_codex_removals: BTreeMap<PathBuf, BTreeSet<String>> = BTreeMap::new();
            let mut stale_claude_removals: BTreeMap<
                (PathBuf, ClaudeJsonTargetLocation),
                BTreeSet<String>,
            > = BTreeMap::new();

            for item in &discovered {
                if !catalog.contains_key(&item.entry.catalog_id) {
                    let managed_elsewhere = catalog.values().any(|e| {
                        e.server_key == item.entry.server_key
                            && e.status == SkillLifecycleStatus::Active
                    });

                    if managed_elsewhere {
                        match item.source {
                            SourceKind::CodexGlobal | SourceKind::ProjectCodex => {
                                stale_codex_removals
                                    .entry(item.file_path.clone())
                                    .or_default()
                                    .insert(item.entry.server_key.clone());
                            }
                            SourceKind::ClaudeUserGlobal => {
                                stale_claude_removals
                                    .entry((item.file_path.clone(), ClaudeJsonTargetLocation::Root))
                                    .or_default()
                                    .insert(item.entry.server_key.clone());
                            }
                            SourceKind::ClaudeUserProject => {
                                if let Some(workspace) = &item.entry.workspace {
                                    stale_claude_removals
                                        .entry((
                                            item.file_path.clone(),
                                            ClaudeJsonTargetLocation::Project {
                                                workspace: workspace.clone(),
                                            },
                                        ))
                                        .or_default()
                                        .insert(item.entry.server_key.clone());
                                }
                            }
                            _ => {
                                warnings.push(format!(
                                    "MCP server '{}' ({}) exists in {} but is unmanaged in central catalog",
                                    item.entry.server_key,
                                    item.entry.catalog_id,
                                    item.file_path.display()
                                ));
                            }
                        }
                    } else {
                        warnings.push(format!(
                            "MCP server '{}' ({}) exists in {} but is unmanaged in central catalog",
                            item.entry.server_key,
                            item.entry.catalog_id,
                            item.file_path.display()
                        ));
                        if let Some(candidate) = build_broken_unmanaged_claude_candidate(item) {
                            warnings
                                .push(format_broken_unmanaged_claude_warning(&candidate.output));
                        }
                    }
                }
            }

            for (path, keys) in &stale_codex_removals {
                for key in keys {
                    if let Err(error) = self.remove_unmanaged_codex_server_entry(path, key) {
                        warnings.push(format!(
                            "Failed to auto-clean stale Codex MCP '{}' from {}: {}",
                            key,
                            path.display(),
                            error
                        ));
                    }
                }
            }

            for ((path, location), keys) in &stale_claude_removals {
                match self.remove_claude_json_server_keys(path, location, keys, &mut warnings) {
                    Ok(count) if count > 0 => {}
                    Ok(_) => {}
                    Err(error) => {
                        warnings.push(format!(
                            "Failed to auto-clean stale Claude MCP entries from {}: {}",
                            path.display(),
                            error
                        ));
                    }
                }
            }
        }
        self.reconcile_claude_enabled(&mut catalog, &discovered, &previous_manifest, &mut warnings);
        let observed_codex = self.load_managed_codex_observed(workspaces, &mut warnings);
        self.reconcile_codex_enabled(
            &mut catalog,
            &observed_codex,
            &previous_manifest,
            &mut warnings,
        );

        self.write_central_catalog(&catalog)?;

        let mut new_manifest = McpManifest {
            version: 3,
            generated_at: iso8601_now(),
            targets: BTreeMap::new(),
            codex_enabled: BTreeMap::new(),
        };

        let global_codex_path = self.codex_config_path();
        let global_codex_entries = codex_entries_for_catalog(
            catalog
                .values()
                .filter(|item| {
                    item.scope == McpScope::Global && item.status == SkillLifecycleStatus::Active
                })
                .cloned()
                .collect::<Vec<_>>()
                .as_slice(),
        );
        let global_codex_keys = self.apply_codex_catalog_path(
            &global_codex_path,
            &global_codex_entries,
            true,
            &mut warnings,
        )?;
        new_manifest
            .targets
            .insert(global_codex_path.display().to_string(), global_codex_keys);
        append_manifest_codex_enabled(
            &mut new_manifest.codex_enabled,
            McpScope::Global,
            None,
            &global_codex_entries,
            new_manifest
                .targets
                .get(&global_codex_path.display().to_string())
                .cloned()
                .unwrap_or_default()
                .as_slice(),
        );

        let global_claude_target = self.effective_global_claude_target_path();
        let claude_user_path = self.claude_user_config_path();

        let mut json_plans: Vec<JsonWritePlan> = Vec::new();
        for item in catalog.values() {
            if item.status == SkillLifecycleStatus::Active
                && item.scope == McpScope::Global
                && item.enabled_by_agent.claude
            {
                push_json_plan_entry(
                    &mut json_plans,
                    &global_claude_target,
                    JsonTargetLocation::Root,
                    true,
                    JsonCatalogEntry {
                        locator: locator_for_entry(item),
                        server_key: item.server_key.clone(),
                        definition: item.definition.clone(),
                    },
                );
            }

            if item.status != SkillLifecycleStatus::Active
                || item.scope != McpScope::Project
                || !item.enabled_by_agent.project
                || !item.enabled_by_agent.claude
            {
                continue;
            }

            let Some(workspace) = item.workspace.as_ref() else {
                continue;
            };
            match item.project_claude_target {
                ProjectClaudeTarget::WorkspaceMcpJson => {
                    let project_path = PathBuf::from(workspace).join(".mcp.json");
                    push_json_plan_entry(
                        &mut json_plans,
                        &project_path,
                        JsonTargetLocation::Root,
                        false,
                        JsonCatalogEntry {
                            locator: locator_for_entry(item),
                            server_key: item.server_key.clone(),
                            definition: item.definition.clone(),
                        },
                    );
                }
                ProjectClaudeTarget::ClaudeUserProject => {
                    push_json_plan_entry(
                        &mut json_plans,
                        &claude_user_path,
                        JsonTargetLocation::Project {
                            workspace: workspace.clone(),
                        },
                        true,
                        JsonCatalogEntry {
                            locator: locator_for_entry(item),
                            server_key: item.server_key.clone(),
                            definition: item.definition.clone(),
                        },
                    );
                }
            }
        }

        let mut touched_json_locations = HashSet::new();
        for plan in &json_plans {
            let path_key = plan.path.display().to_string();
            let previous_for_path = previous_manifest
                .targets
                .get(&path_key)
                .cloned()
                .unwrap_or_default();
            let previous_for_location = filter_manifest_locators_for_location(
                &plan.path,
                &plan.location,
                &previous_for_path,
            );

            if !plan.path.exists() && !plan.create_when_missing {
                if !plan.entries.is_empty() || !previous_for_location.is_empty() {
                    warnings.push(format!(
                        "Skipped project MCP target {} because file does not exist",
                        plan.path.display()
                    ));
                }
                continue;
            }

            let locators = self.apply_json_catalog_path(
                &plan.path,
                &plan.location,
                &plan.entries,
                previous_for_location,
                plan.create_when_missing,
                &mut warnings,
            )?;
            append_manifest_targets(&mut new_manifest.targets, &path_key, locators);
            touched_json_locations.insert(json_location_key(&path_key, &plan.location));
        }

        for (path_key, locators) in &previous_manifest.targets {
            if !path_key.ends_with(".json") {
                continue;
            }
            let path = PathBuf::from(path_key);
            let groups = group_locators_by_location(&path, locators);
            for (location, group_locators) in groups {
                let location_key = json_location_key(path_key, &location);
                if touched_json_locations.contains(&location_key) {
                    continue;
                }
                let _ = self.apply_json_catalog_path(
                    &path,
                    &location,
                    &[],
                    group_locators,
                    false,
                    &mut warnings,
                )?;
            }
        }

        for workspace in workspaces {
            let workspace_key = workspace.display().to_string();
            let project_codex_path = workspace.join(".codex").join("config.toml");
            let previous_project_codex_keys = previous_manifest
                .targets
                .get(&project_codex_path.display().to_string())
                .cloned()
                .unwrap_or_default();
            let project_codex_entries = codex_entries_for_catalog(
                catalog
                    .values()
                    .filter(|item| {
                        item.scope == McpScope::Project
                            && item.status == SkillLifecycleStatus::Active
                            && item.workspace.as_deref() == Some(workspace_key.as_str())
                            && item.enabled_by_agent.project
                    })
                    .cloned()
                    .collect::<Vec<_>>()
                    .as_slice(),
            );
            let project_has_enabled_codex_entries =
                project_codex_entries.values().any(|item| item.enabled);
            if project_codex_path.exists() {
                let keys = self.apply_codex_catalog_path(
                    &project_codex_path,
                    &project_codex_entries,
                    false,
                    &mut warnings,
                )?;
                new_manifest
                    .targets
                    .insert(project_codex_path.display().to_string(), keys);
                append_manifest_codex_enabled(
                    &mut new_manifest.codex_enabled,
                    McpScope::Project,
                    Some(workspace_key.as_str()),
                    &project_codex_entries,
                    new_manifest
                        .targets
                        .get(&project_codex_path.display().to_string())
                        .cloned()
                        .unwrap_or_default()
                        .as_slice(),
                );
            } else if project_has_enabled_codex_entries || !previous_project_codex_keys.is_empty() {
                warnings.push(format!(
                    "Skipped project MCP target {} because file does not exist",
                    project_codex_path.display()
                ));
            }
        }

        self.save_manifest(&new_manifest)?;

        let mut records = catalog
            .into_values()
            .map(|entry| {
                let targets = build_targets(
                    &self.codex_config_path(),
                    &global_claude_target,
                    &claude_user_path,
                    &entry,
                );
                let mut record_warnings =
                    detect_inline_secret_warnings(&entry.server_key, &entry.definition);
                warnings.extend(record_warnings.clone());
                let CatalogEntry {
                    server_key,
                    scope,
                    workspace,
                    definition,
                    enabled_by_agent,
                    status,
                    archived_at,
                    ..
                } = entry;
                McpServerRecord {
                    server_key,
                    scope: scope.as_str().to_string(),
                    workspace,
                    transport: definition.transport,
                    command: definition.command,
                    args: definition.args,
                    url: definition.url,
                    env: definition.env,
                    enabled_by_agent,
                    targets: if status == SkillLifecycleStatus::Active {
                        targets
                    } else {
                        Vec::new()
                    },
                    warnings: {
                        record_warnings.sort();
                        record_warnings.dedup();
                        record_warnings
                    },
                    status,
                    archived_at,
                }
            })
            .collect::<Vec<_>>();
        records.sort_by(|lhs, rhs| {
            lhs.status
                .cmp(&rhs.status)
                .then_with(|| lhs.server_key.cmp(&rhs.server_key))
                .then_with(|| lhs.scope.cmp(&rhs.scope))
                .then_with(|| lhs.workspace.cmp(&rhs.workspace))
        });

        warnings.sort();
        warnings.dedup();

        Ok(McpSyncOutcome { records, warnings })
    }

    pub fn set_enabled(
        &self,
        workspaces: &[PathBuf],
        server_key: &str,
        agent: McpAgent,
        enabled: bool,
        scope: Option<&str>,
        workspace: Option<&str>,
    ) -> Result<(), SyncEngineError> {
        let mut warnings = Vec::new();
        let discovered = self.discover_all(workspaces, &mut warnings);
        let mut catalog = self.load_central_catalog(&mut warnings)?;
        if catalog.is_empty() {
            catalog = self.bootstrap_catalog(&discovered, &mut warnings);
        }

        let scope_filter = scope
            .map(McpScope::parse)
            .transpose()
            .map_err(SyncEngineError::Unsupported)?;
        let workspace_filter = workspace
            .map(|value| value.trim())
            .filter(|value| !value.is_empty());

        let matches = catalog
            .values()
            .filter(|item| {
                item.server_key == server_key
                    && item.status == SkillLifecycleStatus::Active
                    && scope_filter
                        .map(|scope| item.scope == scope)
                        .unwrap_or(true)
                    && workspace_filter
                        .map(|workspace| item.workspace.as_deref() == Some(workspace))
                        .unwrap_or(true)
            })
            .map(|item| item.catalog_id.clone())
            .collect::<Vec<_>>();

        if matches.is_empty() {
            return Err(SyncEngineError::Unsupported(format!(
                "mcp server not found: {server_key}"
            )));
        }
        if matches.len() > 1 {
            return Err(SyncEngineError::Unsupported(format!(
                "ambiguous mcp server locator for '{server_key}', provide scope/workspace"
            )));
        }

        let catalog_id = matches[0].clone();
        let Some(entry) = catalog.get_mut(&catalog_id) else {
            return Err(SyncEngineError::Unsupported(format!(
                "mcp server not found: {server_key}"
            )));
        };

        if entry.scope == McpScope::Global && agent == McpAgent::Project {
            return Err(SyncEngineError::Unsupported(format!(
                "project toggle is unsupported for global mcp server: {server_key}"
            )));
        }

        match agent {
            McpAgent::Codex => entry.enabled_by_agent.codex = enabled,
            McpAgent::Claude => entry.enabled_by_agent.claude = enabled,
            McpAgent::Project => entry.enabled_by_agent.project = enabled,
        }

        if entry.scope == McpScope::Global {
            entry.enabled_by_agent.project = false;
        }

        self.write_central_catalog(&catalog)
    }

    pub fn mutate_catalog_entry(
        &self,
        workspaces: &[PathBuf],
        action: CatalogMutationAction,
        server_key: &str,
        scope: &str,
        workspace: Option<&str>,
    ) -> Result<(), SyncEngineError> {
        let mut warnings = Vec::new();
        let discovered = self.discover_all(workspaces, &mut warnings);
        let mut catalog = self.load_central_catalog(&mut warnings)?;
        if catalog.is_empty() {
            catalog = self.bootstrap_catalog(&discovered, &mut warnings);
        }

        let scope_value =
            McpScope::parse(scope).map_err(|_| SyncEngineError::McpMutationInvalidScope {
                scope: scope.to_string(),
            })?;
        let workspace_filter = workspace
            .map(|value| value.trim())
            .filter(|value| !value.is_empty());

        let matches = catalog
            .values()
            .filter(|item| {
                if item.server_key != server_key || item.scope != scope_value {
                    return false;
                }
                if item.scope == McpScope::Project {
                    if let Some(workspace_filter) = workspace_filter {
                        return item.workspace.as_deref() == Some(workspace_filter);
                    }
                }
                true
            })
            .map(|item| item.catalog_id.clone())
            .collect::<Vec<_>>();

        if matches.is_empty() {
            return Err(SyncEngineError::McpCatalogEntryNotFound {
                server_key: server_key.to_string(),
                scope: scope.to_string(),
            });
        }
        if matches.len() > 1 {
            return Err(SyncEngineError::McpCatalogEntryAmbiguous {
                server_key: server_key.to_string(),
                scope: scope.to_string(),
            });
        }

        let catalog_id = matches[0].clone();
        match action {
            CatalogMutationAction::Delete => {
                catalog.remove(&catalog_id);
            }
            CatalogMutationAction::Archive => {
                let Some(entry) = catalog.get_mut(&catalog_id) else {
                    return Err(SyncEngineError::McpCatalogEntryNotFound {
                        server_key: server_key.to_string(),
                        scope: scope.to_string(),
                    });
                };
                if entry.status != SkillLifecycleStatus::Active {
                    return Err(SyncEngineError::McpArchiveOnlyForActive);
                }
                entry.status = SkillLifecycleStatus::Archived;
                entry.archived_at = Some(iso8601_now());
            }
            CatalogMutationAction::Restore => {
                let Some(entry) = catalog.get_mut(&catalog_id) else {
                    return Err(SyncEngineError::McpCatalogEntryNotFound {
                        server_key: server_key.to_string(),
                        scope: scope.to_string(),
                    });
                };
                if entry.status != SkillLifecycleStatus::Archived {
                    return Err(SyncEngineError::McpRestoreOnlyForArchived);
                }
                entry.status = SkillLifecycleStatus::Active;
                entry.archived_at = None;
            }
            CatalogMutationAction::MakeGlobal => {
                let (entry_server_key, entry_status, entry_scope) = {
                    let Some(entry) = catalog.get(&catalog_id) else {
                        return Err(SyncEngineError::McpCatalogEntryNotFound {
                            server_key: server_key.to_string(),
                            scope: scope.to_string(),
                        });
                    };
                    (entry.server_key.clone(), entry.status, entry.scope)
                };
                if entry_status != SkillLifecycleStatus::Active {
                    return Err(SyncEngineError::McpMakeGlobalOnlyForActive);
                }
                if entry_scope != McpScope::Project {
                    return Err(SyncEngineError::McpMakeGlobalOnlyForProject);
                }

                let global_catalog_id =
                    make_catalog_id(McpScope::Global, None, entry_server_key.as_str());
                if catalog.contains_key(&global_catalog_id) {
                    return Err(SyncEngineError::McpMakeGlobalTargetExists {
                        server_key: entry_server_key,
                    });
                }

                let Some(mut entry) = catalog.remove(&catalog_id) else {
                    return Err(SyncEngineError::McpCatalogEntryNotFound {
                        server_key: server_key.to_string(),
                        scope: scope.to_string(),
                    });
                };
                entry.scope = McpScope::Global;
                entry.workspace = None;
                entry.catalog_id = global_catalog_id.clone();
                entry.enabled_by_agent.project = false;
                entry.archived_at = None;
                catalog.insert(global_catalog_id, entry);
            }
        }

        self.write_central_catalog(&catalog)
    }

    pub fn fix_unmanaged_claude_mcp(
        &self,
        workspaces: &[PathBuf],
        apply: bool,
    ) -> Result<UnmanagedClaudeMcpFixReport, SyncEngineError> {
        let mut warnings = Vec::new();
        let discovered = self.discover_all(workspaces, &mut warnings);
        let catalog = self.load_central_catalog(&mut warnings)?;
        let mut candidates = collect_broken_unmanaged_claude_candidates(&discovered, &catalog);

        let mut removed_count = 0usize;
        let mut changed_files = Vec::new();
        if apply {
            let outcome =
                self.remove_broken_unmanaged_claude_entries(&candidates, &mut warnings)?;
            removed_count = outcome.removed_count;
            changed_files = outcome.changed_files;
            candidates = collect_broken_unmanaged_claude_candidates(
                &self.discover_all(workspaces, &mut warnings),
                &catalog,
            );
        }

        warnings.sort();
        warnings.dedup();

        Ok(UnmanagedClaudeMcpFixReport {
            apply,
            candidates: candidates.into_iter().map(|item| item.output).collect(),
            removed_count,
            changed_files,
            warnings,
        })
    }

    pub fn fix_unmanaged_claude_mcp_warning(
        &self,
        workspaces: &[PathBuf],
        warning: &str,
        apply: bool,
    ) -> Result<UnmanagedClaudeMcpFixReport, SyncEngineError> {
        let mut warnings = Vec::new();
        let discovered = self.discover_all(workspaces, &mut warnings);
        let catalog = self.load_central_catalog(&mut warnings)?;
        let candidates = collect_broken_unmanaged_claude_candidates(&discovered, &catalog);
        let targeted = candidates
            .into_iter()
            .filter(|candidate| {
                format_broken_unmanaged_claude_warning(&candidate.output) == warning
            })
            .collect::<Vec<_>>();

        if targeted.is_empty() {
            return Err(SyncEngineError::Unsupported(format!(
                "warning is not a fixable broken unmanaged Claude MCP warning: {warning}"
            )));
        }

        let mut removed_count = 0usize;
        let mut changed_files = Vec::new();
        if apply {
            let outcome = self.remove_broken_unmanaged_claude_entries(&targeted, &mut warnings)?;
            removed_count = outcome.removed_count;
            changed_files = outcome.changed_files;
        }

        warnings.sort();
        warnings.dedup();

        Ok(UnmanagedClaudeMcpFixReport {
            apply,
            candidates: targeted.into_iter().map(|item| item.output).collect(),
            removed_count,
            changed_files,
            warnings,
        })
    }

    pub fn fix_sync_warning(
        &self,
        workspaces: &[PathBuf],
        warning: &str,
    ) -> Result<(), SyncEngineError> {
        if warning.starts_with("Broken unmanaged Claude MCP '") {
            self.fix_unmanaged_claude_mcp_warning(workspaces, warning, true)
                .map(|_| ())?;
            return Ok(());
        }

        let Some(action) = parse_warning_fix_action(warning) else {
            return Err(SyncEngineError::Unsupported(format!(
                "warning is not fixable: {warning}"
            )));
        };

        match action {
            WarningFixAction::UnmanagedInCentral {
                server_key,
                catalog_id,
                file_path,
            } => self.fix_unmanaged_in_central_warning(
                workspaces,
                &server_key,
                &catalog_id,
                &file_path,
            ),
            WarningFixAction::InlineSecretEnv {
                server_key,
                env_key,
            } => self.fix_inline_secret_env_warning(&server_key, &env_key),
            WarningFixAction::InlineSecretArgument {
                server_key,
                redacted_argument,
            } => self.fix_inline_secret_argument_warning(&server_key, &redacted_argument),
            WarningFixAction::SkippedManagedCodex {
                server_key,
                file_path,
            } => self.fix_skipped_managed_codex_warning(&server_key, &file_path),
            WarningFixAction::SkippedMissingProjectTarget { file_path } => {
                self.fix_missing_project_target_warning(&file_path)
            }
        }
    }

    fn fix_unmanaged_in_central_warning(
        &self,
        workspaces: &[PathBuf],
        server_key: &str,
        catalog_id: &str,
        file_path: &str,
    ) -> Result<(), SyncEngineError> {
        let mut warnings = Vec::new();
        let discovered = self.discover_all(workspaces, &mut warnings);
        let mut catalog = self.load_central_catalog(&mut warnings)?;
        if catalog.contains_key(catalog_id) {
            return Err(SyncEngineError::Unsupported(format!(
                "warning is stale (catalog entry already exists): {catalog_id}"
            )));
        }

        let matched = discovered
            .iter()
            .find(|item| {
                item.entry.server_key == server_key
                    && item.entry.catalog_id == catalog_id
                    && item.file_path.display().to_string() == file_path
            })
            .ok_or_else(|| {
                SyncEngineError::Unsupported(format!(
                    "warning is stale (matching unmanaged MCP entry not found): {server_key} ({catalog_id})"
                ))
            })?;

        let mut related = discovered
            .iter()
            .filter(|item| item.entry.catalog_id == catalog_id)
            .collect::<Vec<_>>();
        related.sort_by(|lhs, rhs| {
            source_priority(lhs.source)
                .cmp(&source_priority(rhs.source))
                .then_with(|| lhs.source.cmp(&rhs.source))
        });
        let Some(primary) = related.first() else {
            return Err(SyncEngineError::Unsupported(format!(
                "warning is stale (related unmanaged MCP entries not found): {catalog_id}"
            )));
        };

        let mut enabled = McpEnabledByAgent {
            codex: false,
            claude: false,
            project: false,
        };
        for item in &related {
            if item.entry.enabled_by_agent.codex && item.enabled {
                enabled.codex = true;
            }
            if item.entry.enabled_by_agent.claude && item.enabled {
                enabled.claude = true;
            }
            if item.entry.enabled_by_agent.project && item.enabled {
                enabled.project = true;
            }
        }
        if primary.entry.scope == McpScope::Global {
            enabled.project = false;
        }

        let mut entry = primary.entry.clone();
        entry.enabled_by_agent = enabled;
        entry.status = SkillLifecycleStatus::Active;
        entry.archived_at = None;
        catalog.insert(entry.catalog_id.clone(), entry);
        self.write_central_catalog(&catalog)?;

        if matches!(
            matched.source,
            SourceKind::CodexGlobal | SourceKind::ProjectCodex
        ) {
            self.remove_unmanaged_codex_server_entry(&matched.file_path, server_key)?;
        }

        Ok(())
    }

    fn fix_inline_secret_env_warning(
        &self,
        server_key: &str,
        env_key: &str,
    ) -> Result<(), SyncEngineError> {
        if !is_non_empty_env_var(env_key) {
            return Err(SyncEngineError::Unsupported(format!(
                "cannot fix inline secret env warning for '{server_key}': environment variable '{env_key}' is missing or empty"
            )));
        }

        let mut warnings = Vec::new();
        let mut catalog = self.load_central_catalog(&mut warnings)?;
        let mut updated_count = 0usize;

        for entry in catalog.values_mut() {
            if entry.server_key != server_key {
                continue;
            }
            let Some(value) = entry.definition.env.get_mut(env_key) else {
                continue;
            };
            if value.starts_with("${") {
                continue;
            }
            *value = format!("${{{env_key}}}");
            updated_count += 1;
        }

        if updated_count == 0 {
            return Err(SyncEngineError::Unsupported(format!(
                "warning is stale (inline secret env key not found): server={server_key}, key={env_key}"
            )));
        }

        self.write_central_catalog(&catalog)
    }

    fn fix_skipped_managed_codex_warning(
        &self,
        server_key: &str,
        file_path: &str,
    ) -> Result<(), SyncEngineError> {
        let removed = self.remove_unmanaged_codex_server_entry(Path::new(file_path), server_key)?;
        if !removed {
            return Err(SyncEngineError::Unsupported(format!(
                "warning is stale (unmanaged codex MCP entry not found): {server_key} in {file_path}"
            )));
        }
        Ok(())
    }

    fn fix_inline_secret_argument_warning(
        &self,
        server_key: &str,
        redacted_argument: &str,
    ) -> Result<(), SyncEngineError> {
        let Some((arg_key, _)) = redacted_argument.split_once('=') else {
            return Err(SyncEngineError::Unsupported(format!(
                "warning is stale (inline secret argument is malformed): server={server_key}, arg={redacted_argument}"
            )));
        };
        let env_key = secret_arg_env_key(arg_key);
        if !is_non_empty_env_var(&env_key) {
            return Err(SyncEngineError::Unsupported(format!(
                "cannot fix inline secret argument warning for '{server_key}': environment variable '{env_key}' is missing or empty"
            )));
        }

        let mut warnings = Vec::new();
        let mut catalog = self.load_central_catalog(&mut warnings)?;
        let mut updated_count = 0usize;

        for entry in catalog.values_mut() {
            if entry.server_key != server_key {
                continue;
            }
            for arg in &mut entry.definition.args {
                let Some(redacted) = redact_secret_like_arg(arg) else {
                    continue;
                };
                if redacted != redacted_argument {
                    continue;
                }
                let Some((arg_key, _)) = arg.split_once('=') else {
                    continue;
                };
                *arg = format!("{arg_key}=${{{env_key}}}");
                updated_count += 1;
            }
        }

        if updated_count == 0 {
            return Err(SyncEngineError::Unsupported(format!(
                "warning is stale (inline secret argument not found): server={server_key}, arg={redacted_argument}"
            )));
        }

        self.write_central_catalog(&catalog)
    }

    fn fix_missing_project_target_warning(&self, file_path: &str) -> Result<(), SyncEngineError> {
        let path = Path::new(file_path);
        if path.exists() {
            return Err(SyncEngineError::Unsupported(format!(
                "warning is stale (project MCP target already exists): {file_path}"
            )));
        }

        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();

        let workspace = if file_name == "config.toml" {
            let codex_dir = path.parent().ok_or_else(|| {
                SyncEngineError::Unsupported(format!(
                    "warning target path is invalid (missing parent): {file_path}"
                ))
            })?;
            if codex_dir.file_name().and_then(|value| value.to_str()) != Some(".codex") {
                return Err(SyncEngineError::Unsupported(format!(
                    "warning target path is unsupported for auto-fix: {file_path}"
                )));
            }
            codex_dir.parent().ok_or_else(|| {
                SyncEngineError::Unsupported(format!(
                    "warning target path is invalid (missing workspace): {file_path}"
                ))
            })?
        } else if file_name == ".mcp.json" {
            path.parent().ok_or_else(|| {
                SyncEngineError::Unsupported(format!(
                    "warning target path is invalid (missing workspace): {file_path}"
                ))
            })?
        } else {
            return Err(SyncEngineError::Unsupported(format!(
                "warning target path is unsupported for auto-fix: {file_path}"
            )));
        };

        if !workspace.exists() {
            return Err(SyncEngineError::Unsupported(format!(
                "workspace does not exist for target path: {}",
                workspace.display()
            )));
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| SyncEngineError::io(parent, error))?;
        }

        if file_name == ".mcp.json" {
            fs::write(path, "{\n  \"mcpServers\": {}\n}\n")
                .map_err(|error| SyncEngineError::io(path, error))?;
        } else {
            fs::write(path, "").map_err(|error| SyncEngineError::io(path, error))?;
        }
        Ok(())
    }

    fn remove_unmanaged_codex_server_entry(
        &self,
        path: &Path,
        server_key: &str,
    ) -> Result<bool, SyncEngineError> {
        let existing = match fs::read_to_string(path) {
            Ok(value) => value,
            Err(error) => {
                if error.kind() == std::io::ErrorKind::NotFound {
                    return Ok(false);
                }
                return Err(SyncEngineError::io(path, error));
            }
        };

        let unmanaged = strip_managed_blocks(&existing, &CODEX_MARKER_PAIRS);
        if !text_contains_codex_server(&unmanaged, server_key) {
            return Ok(false);
        }

        let unmanaged_text = text_remove_codex_server_section(&unmanaged, server_key);

        let updated = if let Some(body) =
            extract_managed_block_from_markers(&existing, &CODEX_MARKER_PAIRS)
        {
            upsert_managed_block(&unmanaged_text, CODEX_BEGIN, CODEX_END, body.trim())
        } else {
            unmanaged_text
        };

        if updated != existing {
            fs::write(path, updated).map_err(|error| SyncEngineError::io(path, error))?;
        }

        Ok(true)
    }

    fn effective_global_claude_target_path(&self) -> PathBuf {
        let user = self.claude_user_config_path();
        if user.exists() {
            return user;
        }
        self.claude_local_settings_path()
    }

    fn reconcile_claude_enabled(
        &self,
        catalog: &mut BTreeMap<String, CatalogEntry>,
        discovered: &[Discovered],
        previous_manifest: &McpManifest,
        warnings: &mut Vec<String>,
    ) {
        for item in discovered {
            if !item.enabled || !item.entry.enabled_by_agent.claude {
                continue;
            }
            let Some(existing) = catalog.get_mut(&item.entry.catalog_id) else {
                continue;
            };
            if existing.status != SkillLifecycleStatus::Active {
                continue;
            }
            if existing.enabled_by_agent.claude {
                continue;
            }
            if was_previously_managed_claude_locator(
                previous_manifest,
                &locator_for_entry(existing),
            ) {
                continue;
            }
            existing.enabled_by_agent.claude = true;
            warnings.push(format!(
                "Auto-aligned MCP '{}' ({}) with observed Claude enabled state from {}",
                existing.server_key,
                existing.catalog_id,
                item.file_path.display()
            ));
        }
    }

    fn load_managed_codex_observed(
        &self,
        workspaces: &[PathBuf],
        warnings: &mut Vec<String>,
    ) -> BTreeMap<String, ManagedCodexObserved> {
        let mut observed = BTreeMap::new();
        let global_path = self.codex_config_path();
        self.collect_managed_codex_observed_for_path(
            &global_path,
            McpScope::Global,
            None,
            &mut observed,
            warnings,
        );
        for workspace in workspaces {
            let project_path = workspace.join(".codex").join("config.toml");
            if !project_path.exists() {
                continue;
            }
            let workspace_key = workspace.display().to_string();
            self.collect_managed_codex_observed_for_path(
                &project_path,
                McpScope::Project,
                Some(workspace_key.as_str()),
                &mut observed,
                warnings,
            );
        }
        observed
    }

    fn collect_managed_codex_observed_for_path(
        &self,
        path: &Path,
        scope: McpScope,
        workspace: Option<&str>,
        observed: &mut BTreeMap<String, ManagedCodexObserved>,
        warnings: &mut Vec<String>,
    ) {
        let raw = match fs::read_to_string(path) {
            Ok(value) => value,
            Err(error) => {
                if error.kind() != std::io::ErrorKind::NotFound {
                    warnings.push(format!(
                        "Failed to read Codex MCP config {}: {error}",
                        path.display()
                    ));
                }
                return;
            }
        };
        let Some(body) = extract_managed_block_from_markers(&raw, &CODEX_MARKER_PAIRS) else {
            return;
        };
        let servers = match read_toml_servers_from_str(body.trim()) {
            Ok(items) => items,
            Err(error) => {
                warnings.push(format!(
                    "Failed to parse managed Codex MCP block {}: {error}",
                    path.display()
                ));
                return;
            }
        };
        for (server_key, _definition, enabled) in servers {
            let catalog_id = make_catalog_id(scope, workspace, &server_key);
            observed.insert(
                catalog_id.clone(),
                ManagedCodexObserved {
                    catalog_id,
                    server_key,
                    enabled,
                    file_path: path.to_path_buf(),
                },
            );
        }
    }

    fn reconcile_codex_enabled(
        &self,
        catalog: &mut BTreeMap<String, CatalogEntry>,
        observed: &BTreeMap<String, ManagedCodexObserved>,
        previous_manifest: &McpManifest,
        warnings: &mut Vec<String>,
    ) {
        for item in observed.values() {
            let Some(existing) = catalog.get_mut(&item.catalog_id) else {
                continue;
            };
            if existing.status != SkillLifecycleStatus::Active {
                continue;
            }
            let locator = locator_for_entry(existing);
            let Some(previous_enabled) = previous_manifest.codex_enabled.get(&locator) else {
                continue;
            };
            if *previous_enabled == item.enabled {
                continue;
            }
            if existing.enabled_by_agent.codex == item.enabled {
                continue;
            }
            existing.enabled_by_agent.codex = item.enabled;
            warnings.push(format!(
                "Auto-aligned MCP '{}' ({}) with observed Codex enabled={} from {}",
                item.server_key,
                item.catalog_id,
                item.enabled,
                item.file_path.display()
            ));
        }
    }

    pub fn managed_watch_paths(&self, workspaces: &[PathBuf]) -> Vec<PathBuf> {
        let mut paths = vec![
            self.central_config_path(),
            self.codex_config_path(),
            self.claude_user_config_path(),
            self.claude_local_settings_path(),
        ];
        for workspace in workspaces {
            let project_codex = workspace.join(".codex").join("config.toml");
            if project_codex.exists() {
                paths.push(project_codex);
            }
            let project_claude = workspace.join(".mcp.json");
            if project_claude.exists() {
                paths.push(project_claude);
            }
        }
        let mut unique: BTreeSet<String> = BTreeSet::new();
        let mut deduped = Vec::new();
        for path in paths {
            let key = path.display().to_string();
            if unique.insert(key) {
                deduped.push(path);
            }
        }
        deduped
    }

    fn discover_all(&self, workspaces: &[PathBuf], warnings: &mut Vec<String>) -> Vec<Discovered> {
        let mut result = Vec::new();
        result.extend(self.load_codex_global_discovered(warnings));
        result.extend(self.load_claude_user_discovered(warnings));
        result.extend(self.load_claude_global_discovered(warnings));

        for workspace in workspaces {
            let workspace_value = workspace.display().to_string();

            let project_codex_path = workspace.join(".codex").join("config.toml");
            if project_codex_path.exists() {
                match read_toml_servers(&project_codex_path) {
                    Ok(items) => {
                        for (server_key, definition, enabled) in items {
                            let catalog_id = make_catalog_id(
                                McpScope::Project,
                                Some(workspace_value.as_str()),
                                &server_key,
                            );
                            result.push(Discovered {
                                source: SourceKind::ProjectCodex,
                                file_path: project_codex_path.clone(),
                                entry: CatalogEntry {
                                    catalog_id,
                                    server_key,
                                    scope: McpScope::Project,
                                    workspace: Some(workspace_value.clone()),
                                    definition,
                                    enabled_by_agent: McpEnabledByAgent {
                                        codex: true,
                                        claude: false,
                                        project: true,
                                    },
                                    project_claude_target: ProjectClaudeTarget::WorkspaceMcpJson,
                                    status: SkillLifecycleStatus::Active,
                                    archived_at: None,
                                },
                                enabled,
                            });
                        }
                    }
                    Err(error) => warnings.push(format!(
                        "Failed to parse project Codex MCP config {}: {error}",
                        project_codex_path.display()
                    )),
                }
            }

            let project_mcp_path = workspace.join(".mcp.json");
            if project_mcp_path.exists() {
                match read_json_servers(&project_mcp_path) {
                    Ok(items) => {
                        for (server_key, definition, enabled) in items {
                            let catalog_id = make_catalog_id(
                                McpScope::Project,
                                Some(workspace_value.as_str()),
                                &server_key,
                            );
                            result.push(Discovered {
                                source: SourceKind::ProjectClaude,
                                file_path: project_mcp_path.clone(),
                                entry: CatalogEntry {
                                    catalog_id,
                                    server_key,
                                    scope: McpScope::Project,
                                    workspace: Some(workspace_value.clone()),
                                    definition,
                                    enabled_by_agent: McpEnabledByAgent {
                                        codex: false,
                                        claude: true,
                                        project: true,
                                    },
                                    project_claude_target: ProjectClaudeTarget::WorkspaceMcpJson,
                                    status: SkillLifecycleStatus::Active,
                                    archived_at: None,
                                },
                                enabled,
                            });
                        }
                    }
                    Err(error) => warnings.push(format!(
                        "Failed to parse project MCP file {}: {error}",
                        project_mcp_path.display()
                    )),
                }
            }
        }

        result
    }

    fn load_codex_global_discovered(&self, warnings: &mut Vec<String>) -> Vec<Discovered> {
        let path = self.codex_config_path();
        match read_toml_servers(&path) {
            Ok(items) => items
                .into_iter()
                .map(|(server_key, definition, enabled)| Discovered {
                    source: SourceKind::CodexGlobal,
                    file_path: path.clone(),
                    entry: CatalogEntry {
                        catalog_id: make_catalog_id(McpScope::Global, None, &server_key),
                        server_key,
                        scope: McpScope::Global,
                        workspace: None,
                        definition,
                        enabled_by_agent: McpEnabledByAgent {
                            codex: true,
                            claude: false,
                            project: false,
                        },
                        project_claude_target: ProjectClaudeTarget::WorkspaceMcpJson,
                        status: SkillLifecycleStatus::Active,
                        archived_at: None,
                    },
                    enabled,
                })
                .collect(),
            Err(error) => {
                if path.exists() {
                    warnings.push(format!(
                        "Failed to parse Codex MCP config {}: {error}",
                        path.display()
                    ));
                }
                Vec::new()
            }
        }
    }

    fn load_claude_user_discovered(&self, warnings: &mut Vec<String>) -> Vec<Discovered> {
        let path = self.claude_user_config_path();
        match read_claude_user_servers(&path) {
            Ok(parsed) => {
                let mut result = Vec::new();
                for (server_key, definition, enabled) in parsed.global {
                    result.push(Discovered {
                        source: SourceKind::ClaudeUserGlobal,
                        file_path: path.clone(),
                        entry: CatalogEntry {
                            catalog_id: make_catalog_id(McpScope::Global, None, &server_key),
                            server_key,
                            scope: McpScope::Global,
                            workspace: None,
                            definition,
                            enabled_by_agent: McpEnabledByAgent {
                                codex: false,
                                claude: true,
                                project: false,
                            },
                            project_claude_target: ProjectClaudeTarget::WorkspaceMcpJson,
                            status: SkillLifecycleStatus::Active,
                            archived_at: None,
                        },
                        enabled,
                    });
                }
                for (workspace, items) in parsed.projects {
                    for (server_key, definition, enabled) in items {
                        result.push(Discovered {
                            source: SourceKind::ClaudeUserProject,
                            file_path: path.clone(),
                            entry: CatalogEntry {
                                catalog_id: make_catalog_id(
                                    McpScope::Project,
                                    Some(workspace.as_str()),
                                    &server_key,
                                ),
                                server_key,
                                scope: McpScope::Project,
                                workspace: Some(workspace.clone()),
                                definition,
                                enabled_by_agent: McpEnabledByAgent {
                                    codex: false,
                                    claude: true,
                                    project: true,
                                },
                                project_claude_target: ProjectClaudeTarget::ClaudeUserProject,
                                status: SkillLifecycleStatus::Active,
                                archived_at: None,
                            },
                            enabled,
                        });
                    }
                }
                result
            }
            Err(error) => {
                if path.exists() {
                    warnings.push(format!(
                        "Failed to parse Claude user MCP config {}: {error}",
                        path.display()
                    ));
                }
                Vec::new()
            }
        }
    }

    fn load_claude_global_discovered(&self, warnings: &mut Vec<String>) -> Vec<Discovered> {
        let mut result = Vec::new();
        let local = self.claude_local_settings_path();
        match read_json_servers(&local) {
            Ok(items) => {
                for (server_key, definition, enabled) in items {
                    result.push(Discovered {
                        source: SourceKind::ClaudeLocalGlobal,
                        file_path: local.clone(),
                        entry: CatalogEntry {
                            catalog_id: make_catalog_id(McpScope::Global, None, &server_key),
                            server_key,
                            scope: McpScope::Global,
                            workspace: None,
                            definition,
                            enabled_by_agent: McpEnabledByAgent {
                                codex: false,
                                claude: true,
                                project: false,
                            },
                            project_claude_target: ProjectClaudeTarget::WorkspaceMcpJson,
                            status: SkillLifecycleStatus::Active,
                            archived_at: None,
                        },
                        enabled,
                    });
                }
            }
            Err(error) => {
                if local.exists() {
                    warnings.push(format!(
                        "Failed to parse Claude MCP config {}: {error}",
                        local.display()
                    ));
                }
            }
        }

        let global = self.claude_global_settings_path();
        match read_json_servers(&global) {
            Ok(items) => {
                for (server_key, definition, enabled) in items {
                    result.push(Discovered {
                        source: SourceKind::ClaudeGlobalGlobal,
                        file_path: global.clone(),
                        entry: CatalogEntry {
                            catalog_id: make_catalog_id(McpScope::Global, None, &server_key),
                            server_key,
                            scope: McpScope::Global,
                            workspace: None,
                            definition,
                            enabled_by_agent: McpEnabledByAgent {
                                codex: false,
                                claude: true,
                                project: false,
                            },
                            project_claude_target: ProjectClaudeTarget::WorkspaceMcpJson,
                            status: SkillLifecycleStatus::Active,
                            archived_at: None,
                        },
                        enabled,
                    });
                }
            }
            Err(error) => {
                if global.exists() {
                    warnings.push(format!(
                        "Failed to parse Claude global MCP config {}: {error}",
                        global.display()
                    ));
                }
            }
        }

        result
    }

    fn bootstrap_catalog(
        &self,
        discovered: &[Discovered],
        warnings: &mut Vec<String>,
    ) -> BTreeMap<String, CatalogEntry> {
        let mut grouped: HashMap<String, Vec<&Discovered>> = HashMap::new();
        for item in discovered {
            grouped
                .entry(item.entry.catalog_id.clone())
                .or_default()
                .push(item);
        }

        let mut result = BTreeMap::new();
        for (catalog_id, items) in grouped {
            let mut sorted = items;
            sorted.sort_by(|lhs, rhs| {
                source_priority(lhs.source)
                    .cmp(&source_priority(rhs.source))
                    .then_with(|| lhs.source.cmp(&rhs.source))
            });
            let Some(primary) = sorted.first() else {
                continue;
            };

            let mut enabled = McpEnabledByAgent {
                codex: false,
                claude: false,
                project: false,
            };
            for item in &sorted {
                if item.entry.enabled_by_agent.codex && item.enabled {
                    enabled.codex = true;
                }
                if item.entry.enabled_by_agent.claude && item.enabled {
                    enabled.claude = true;
                }
                if item.entry.enabled_by_agent.project && item.enabled {
                    enabled.project = true;
                }
            }
            if primary.entry.scope == McpScope::Global {
                enabled.project = false;
            }

            for item in sorted.iter().skip(1) {
                if item.entry.definition != primary.entry.definition {
                    warnings.push(format!(
                        "MCP definition conflict for '{}': {} overrides {}",
                        catalog_id,
                        source_label(primary.source),
                        source_label(item.source)
                    ));
                }
            }

            result.insert(
                catalog_id.clone(),
                CatalogEntry {
                    catalog_id,
                    server_key: primary.entry.server_key.clone(),
                    scope: primary.entry.scope,
                    workspace: primary.entry.workspace.clone(),
                    definition: primary.entry.definition.clone(),
                    enabled_by_agent: enabled,
                    project_claude_target: match primary.source {
                        SourceKind::ClaudeUserProject => ProjectClaudeTarget::ClaudeUserProject,
                        _ => ProjectClaudeTarget::WorkspaceMcpJson,
                    },
                    status: SkillLifecycleStatus::Active,
                    archived_at: None,
                },
            );
        }

        result
    }

    fn load_central_catalog(
        &self,
        warnings: &mut Vec<String>,
    ) -> Result<BTreeMap<String, CatalogEntry>, SyncEngineError> {
        let path = self.central_config_path();
        let raw = match fs::read_to_string(&path) {
            Ok(value) => value,
            Err(error) => {
                if error.kind() == std::io::ErrorKind::NotFound {
                    return Ok(BTreeMap::new());
                }
                return Err(SyncEngineError::io(path, error));
            }
        };

        let Some(body) = extract_managed_block_from_markers(&raw, &CENTRAL_MARKER_PAIRS) else {
            return Ok(BTreeMap::new());
        };
        if body.trim().is_empty() {
            return Ok(BTreeMap::new());
        }

        let parsed: toml::Table = toml::from_str(body.trim()).map_err(|error| {
            SyncEngineError::Unsupported(format!("invalid central MCP block: {error}"))
        })?;
        let Some(root) = parsed.get("mcp_catalog") else {
            return Ok(BTreeMap::new());
        };
        let Some(catalog) = root.as_table() else {
            return Ok(BTreeMap::new());
        };

        let mut result = BTreeMap::new();
        for (raw_catalog_id, value) in catalog {
            let Some(table) = value.as_table() else {
                continue;
            };

            let inferred = infer_scope_workspace_and_server(raw_catalog_id);
            let scope_value = table
                .get("scope")
                .and_then(toml::Value::as_str)
                .or(inferred.scope);
            let Some(scope_raw) = scope_value else {
                warnings.push(format!(
                    "Skipped central MCP entry '{}' because scope is missing",
                    raw_catalog_id
                ));
                continue;
            };
            let scope = match McpScope::parse(scope_raw) {
                Ok(value) => value,
                Err(error) => {
                    warnings.push(format!(
                        "Skipped central MCP entry '{}': {}",
                        raw_catalog_id, error
                    ));
                    continue;
                }
            };

            let workspace = table
                .get("workspace")
                .and_then(toml::Value::as_str)
                .map(ToString::to_string)
                .or_else(|| inferred.workspace.map(ToString::to_string));

            let server_key = table
                .get("server_key")
                .and_then(toml::Value::as_str)
                .map(ToString::to_string)
                .or_else(|| inferred.server_key.map(ToString::to_string))
                .unwrap_or_else(|| raw_catalog_id.to_string());

            if scope == McpScope::Project && workspace.is_none() {
                warnings.push(format!(
                    "Skipped central MCP entry '{}' because project scope requires workspace",
                    raw_catalog_id
                ));
                continue;
            }

            let catalog_id = make_catalog_id(scope, workspace.as_deref(), &server_key);
            let definition = definition_from_toml_table(table);
            let mut enabled = enabled_from_toml_table(table);
            let mut project_claude_target = table
                .get("project_claude_target")
                .and_then(toml::Value::as_str)
                .and_then(ProjectClaudeTarget::parse)
                .unwrap_or(ProjectClaudeTarget::WorkspaceMcpJson);
            let status = match table.get("status").and_then(toml::Value::as_str) {
                Some("archived") => SkillLifecycleStatus::Archived,
                Some("active") | None => SkillLifecycleStatus::Active,
                Some(other) => {
                    warnings.push(format!(
                        "Skipped central MCP entry '{}' because status is invalid: {}",
                        raw_catalog_id, other
                    ));
                    continue;
                }
            };
            let archived_at = table
                .get("archived_at")
                .and_then(toml::Value::as_str)
                .map(ToString::to_string);
            if scope == McpScope::Global {
                enabled.project = false;
                project_claude_target = ProjectClaudeTarget::WorkspaceMcpJson;
            }
            if definition.command.is_none() && definition.url.is_none() {
                warnings.push(format!(
                    "Central MCP server '{}' has neither command nor url",
                    server_key
                ));
            }

            result.insert(
                catalog_id.clone(),
                CatalogEntry {
                    catalog_id,
                    server_key,
                    scope,
                    workspace,
                    definition,
                    enabled_by_agent: enabled,
                    project_claude_target,
                    status,
                    archived_at: if status == SkillLifecycleStatus::Archived {
                        archived_at
                    } else {
                        None
                    },
                },
            );
        }

        Ok(result)
    }

    fn write_central_catalog(
        &self,
        catalog: &BTreeMap<String, CatalogEntry>,
    ) -> Result<(), SyncEngineError> {
        let path = self.central_config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| SyncEngineError::io(parent, error))?;
        }
        let existing = fs::read_to_string(&path).unwrap_or_default();
        let unmanaged = strip_managed_blocks(&existing, &CENTRAL_MARKER_PAIRS);
        let block = render_central_block(catalog);
        let updated = upsert_managed_block(&unmanaged, CENTRAL_BEGIN, CENTRAL_END, &block);
        if updated == existing {
            return Ok(());
        }
        fs::write(&path, updated).map_err(|error| SyncEngineError::io(&path, error))
    }

    fn apply_codex_catalog_path(
        &self,
        path: &Path,
        entries: &BTreeMap<String, CodexCatalogEntry>,
        create_when_missing: bool,
        warnings: &mut Vec<String>,
    ) -> Result<Vec<String>, SyncEngineError> {
        let existing = match fs::read_to_string(path) {
            Ok(value) => value,
            Err(error) => {
                if error.kind() == std::io::ErrorKind::NotFound && create_when_missing {
                    String::new()
                } else if error.kind() == std::io::ErrorKind::NotFound {
                    return Ok(Vec::new());
                } else {
                    return Err(SyncEngineError::io(path, error));
                }
            }
        };

        let mut unmanaged_text = strip_managed_blocks(&existing, &CODEX_MARKER_PAIRS);
        let unmanaged_table = parse_unmanaged_codex_table(&unmanaged_text, path, warnings);
        let mut filtered = BTreeMap::new();
        for (key, entry) in entries {
            let has_unmanaged_duplicate = unmanaged_table
                .as_ref()
                .map(|table| unmanaged_codex_contains_server(table, key))
                .unwrap_or_else(|| text_contains_codex_server(&unmanaged_text, key));
            if !entry.enabled {
                if has_unmanaged_duplicate {
                    unmanaged_text = text_remove_codex_server_section(&unmanaged_text, key);
                    warnings.push(format!(
                        "Auto-cleaned unmanaged Codex MCP '{}' because managed entry is disabled in {}",
                        key,
                        path.display()
                    ));
                }
                filtered.insert(key.clone(), entry.clone());
                continue;
            }

            if has_unmanaged_duplicate {
                warnings.push(format!(
                    "Skipped managed Codex MCP '{}' because unmanaged entry already exists in {}",
                    key,
                    path.display()
                ));
                continue;
            }
            filtered.insert(key.clone(), entry.clone());
        }

        if let Some(parent) = path.parent() {
            if create_when_missing || path.exists() {
                fs::create_dir_all(parent).map_err(|error| SyncEngineError::io(parent, error))?;
            }
        }

        // Defensive deduplication: remove from managed block any key that still
        // exists in unmanaged text (catches edge cases the primary check may miss).
        let filtered = {
            let mut safe = filtered;
            let keys_to_check: Vec<_> = safe
                .keys()
                .filter(|k| safe.get(*k).is_some_and(|e| e.enabled))
                .cloned()
                .collect();
            for key in keys_to_check {
                let still_in_unmanaged = unmanaged_table
                    .as_ref()
                    .map(|table| unmanaged_codex_contains_server(table, &key))
                    .unwrap_or_else(|| text_contains_codex_server(&unmanaged_text, &key));
                if still_in_unmanaged {
                    safe.remove(&key);
                    warnings.push(format!(
                        "Defensive dedup: removed managed Codex MCP '{}' that duplicates unmanaged entry in {}",
                        key,
                        path.display()
                    ));
                }
            }
            safe
        };

        let block = render_codex_block(&filtered);
        let mut updated = upsert_managed_block(&unmanaged_text, CODEX_BEGIN, CODEX_END, &block);

        // Post-write TOML validation: if the combined output has duplicate keys,
        // strip duplicates from the managed block to produce valid TOML.
        if toml::from_str::<toml::Table>(&updated).is_err() {
            let managed_only_keys: Vec<_> = filtered.keys().cloned().collect();
            let mut safe_filtered = filtered.clone();
            for key in &managed_only_keys {
                if text_contains_codex_server(&unmanaged_text, key) {
                    safe_filtered.remove(key);
                    warnings.push(format!(
                        "TOML validation fix: removed duplicate managed MCP '{}' in {}",
                        key,
                        path.display()
                    ));
                }
            }
            if safe_filtered.len() != filtered.len() {
                let safe_block = render_codex_block(&safe_filtered);
                updated =
                    upsert_managed_block(&unmanaged_text, CODEX_BEGIN, CODEX_END, &safe_block);
            }
        }

        if updated != existing {
            fs::write(path, &updated).map_err(|error| SyncEngineError::io(path, error))?;
        }

        Ok(filtered.keys().cloned().collect())
    }

    fn apply_json_catalog_path(
        &self,
        path: &Path,
        location: &JsonTargetLocation,
        entries: &[JsonCatalogEntry],
        previous_locators: Vec<String>,
        create_when_missing: bool,
        warnings: &mut Vec<String>,
    ) -> Result<Vec<String>, SyncEngineError> {
        let existing_raw = match fs::read_to_string(path) {
            Ok(value) => value,
            Err(error) => {
                if error.kind() == std::io::ErrorKind::NotFound && create_when_missing {
                    String::new()
                } else if error.kind() == std::io::ErrorKind::NotFound {
                    return Ok(Vec::new());
                } else {
                    return Err(SyncEngineError::io(path, error));
                }
            }
        };

        let mut root = if existing_raw.trim().is_empty() {
            JsonValue::Object(JsonMap::new())
        } else {
            match serde_json::from_str::<JsonValue>(&existing_raw) {
                Ok(value) => value,
                Err(error) => {
                    warnings.push(format!(
                        "Failed to parse JSON MCP target {}: {error}",
                        path.display()
                    ));
                    return Ok(Vec::new());
                }
            }
        };

        let Some(root_obj) = root.as_object_mut() else {
            warnings.push(format!(
                "Skipped non-object JSON MCP target {}",
                path.display()
            ));
            return Ok(Vec::new());
        };

        let mcp_servers = match location {
            JsonTargetLocation::Root => {
                let mcp_value = root_obj
                    .entry("mcpServers".to_string())
                    .or_insert_with(|| JsonValue::Object(JsonMap::new()));
                let Some(mcp_servers) = mcp_value.as_object_mut() else {
                    warnings.push(format!(
                        "Skipped JSON MCP target {} because mcpServers is not an object",
                        path.display()
                    ));
                    return Ok(Vec::new());
                };
                mcp_servers
            }
            JsonTargetLocation::Project { workspace } => {
                let projects_value = root_obj
                    .entry("projects".to_string())
                    .or_insert_with(|| JsonValue::Object(JsonMap::new()));
                let Some(projects) = projects_value.as_object_mut() else {
                    warnings.push(format!(
                        "Skipped JSON MCP target {} because projects is not an object",
                        path.display()
                    ));
                    return Ok(Vec::new());
                };
                let project_value = projects
                    .entry(workspace.clone())
                    .or_insert_with(|| JsonValue::Object(JsonMap::new()));
                let Some(project_obj) = project_value.as_object_mut() else {
                    warnings.push(format!(
                        "Skipped JSON MCP target {} because projects.{} is not an object",
                        path.display(),
                        workspace
                    ));
                    return Ok(Vec::new());
                };
                let project_mcp = project_obj
                    .entry("mcpServers".to_string())
                    .or_insert_with(|| JsonValue::Object(JsonMap::new()));
                let Some(mcp_servers) = project_mcp.as_object_mut() else {
                    warnings.push(format!(
                        "Skipped JSON MCP target {} because projects.{}.mcpServers is not an object",
                        path.display(),
                        workspace
                    ));
                    return Ok(Vec::new());
                };
                mcp_servers
            }
        };

        let enabled_locators = entries
            .iter()
            .map(|item| item.locator.clone())
            .collect::<Vec<_>>();
        let enabled_keys = entries
            .iter()
            .map(|item| item.server_key.as_str())
            .collect::<HashSet<_>>();
        for locator in previous_locators {
            let key = locator_server_key(&locator).unwrap_or(locator.as_str());
            if !enabled_keys.contains(key) {
                mcp_servers.remove(key);
            }
        }

        for entry in entries {
            mcp_servers.insert(
                entry.server_key.clone(),
                definition_to_json(&entry.definition),
            );
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| SyncEngineError::io(parent, error))?;
        }

        let rendered = render_json_pretty(&root).map_err(SyncEngineError::Json)?;
        let next_raw = String::from_utf8_lossy(&rendered).to_string();
        if next_raw != existing_raw {
            fs::write(path, rendered).map_err(|error| SyncEngineError::io(path, error))?;
        }

        Ok(enabled_locators)
    }

    fn load_manifest(&self) -> McpManifest {
        let path = self.manifest_path();
        let Ok(raw) = fs::read(&path) else {
            return McpManifest::default();
        };
        serde_json::from_slice(&raw).unwrap_or_default()
    }

    fn save_manifest(&self, manifest: &McpManifest) -> Result<(), SyncEngineError> {
        let path = self.manifest_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| SyncEngineError::io(parent, error))?;
        }
        write_json_pretty(&path, manifest)
    }

    fn remove_claude_json_server_keys(
        &self,
        path: &Path,
        location: &ClaudeJsonTargetLocation,
        keys: &BTreeSet<String>,
        warnings: &mut Vec<String>,
    ) -> Result<usize, SyncEngineError> {
        let raw = match fs::read_to_string(path) {
            Ok(value) => value,
            Err(error) => {
                if error.kind() == std::io::ErrorKind::NotFound {
                    warnings.push(format!(
                        "Skipped unmanaged Claude MCP repair for {} because file is missing",
                        path.display()
                    ));
                    return Ok(0);
                }
                return Err(SyncEngineError::io(path, error));
            }
        };
        if raw.trim().is_empty() {
            return Ok(0);
        }

        let mut root = match serde_json::from_str::<JsonValue>(&raw) {
            Ok(value) => value,
            Err(error) => {
                warnings.push(format!(
                    "Skipped unmanaged Claude MCP repair for {} because JSON parse failed: {error}",
                    path.display()
                ));
                return Ok(0);
            }
        };

        let Some(root_obj) = root.as_object_mut() else {
            warnings.push(format!(
                "Skipped unmanaged Claude MCP repair for {} because root is not an object",
                path.display()
            ));
            return Ok(0);
        };

        let removed_from_target = match location {
            ClaudeJsonTargetLocation::Root => {
                let Some(mcp_servers) = root_obj.get_mut("mcpServers") else {
                    return Ok(0);
                };
                let Some(server_map) = mcp_servers.as_object_mut() else {
                    warnings.push(format!(
                        "Skipped unmanaged Claude MCP repair for {} because mcpServers is not an object",
                        path.display()
                    ));
                    return Ok(0);
                };
                remove_server_keys(server_map, keys)
            }
            ClaudeJsonTargetLocation::Project { workspace } => {
                let Some(projects) = root_obj.get_mut("projects") else {
                    return Ok(0);
                };
                let Some(projects_map) = projects.as_object_mut() else {
                    warnings.push(format!(
                        "Skipped unmanaged Claude MCP repair for {} because projects is not an object",
                        path.display()
                    ));
                    return Ok(0);
                };
                let Some(project) = projects_map.get_mut(workspace) else {
                    return Ok(0);
                };
                let Some(project_obj) = project.as_object_mut() else {
                    warnings.push(format!(
                        "Skipped unmanaged Claude MCP repair for {} because projects.{} is not an object",
                        path.display(),
                        workspace
                    ));
                    return Ok(0);
                };
                let Some(mcp_servers) = project_obj.get_mut("mcpServers") else {
                    return Ok(0);
                };
                let Some(server_map) = mcp_servers.as_object_mut() else {
                    warnings.push(format!(
                        "Skipped unmanaged Claude MCP repair for {} because projects.{}.mcpServers is not an object",
                        path.display(),
                        workspace
                    ));
                    return Ok(0);
                };
                remove_server_keys(server_map, keys)
            }
        };

        if removed_from_target == 0 {
            return Ok(0);
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| SyncEngineError::io(parent, error))?;
        }

        write_json_pretty(path, &root)?;

        Ok(removed_from_target)
    }

    fn remove_broken_unmanaged_claude_entries(
        &self,
        candidates: &[BrokenUnmanagedClaudeCandidate],
        warnings: &mut Vec<String>,
    ) -> Result<RemovalOutcome, SyncEngineError> {
        let mut grouped: BTreeMap<(PathBuf, ClaudeJsonTargetLocation), BTreeSet<String>> =
            BTreeMap::new();
        for candidate in candidates {
            grouped
                .entry((candidate.path.clone(), candidate.location.clone()))
                .or_default()
                .insert(candidate.server_key.clone());
        }

        let mut changed_files = BTreeSet::new();
        let mut removed_count = 0usize;

        for ((path, location), keys) in &grouped {
            let removed = self.remove_claude_json_server_keys(path, location, keys, warnings)?;
            if removed > 0 {
                removed_count += removed;
                changed_files.insert(path.display().to_string());
            }
        }

        Ok(RemovalOutcome {
            removed_count,
            changed_files: changed_files.into_iter().collect(),
        })
    }

    fn central_config_path(&self) -> PathBuf {
        self.home_directory
            .join(".config")
            .join("ai-agents")
            .join("config.toml")
    }

    fn codex_config_path(&self) -> PathBuf {
        self.home_directory.join(".codex").join("config.toml")
    }

    fn claude_local_settings_path(&self) -> PathBuf {
        self.home_directory
            .join(".claude")
            .join("settings.local.json")
    }

    fn claude_user_config_path(&self) -> PathBuf {
        self.home_directory.join(".claude.json")
    }

    fn claude_global_settings_path(&self) -> PathBuf {
        self.home_directory.join(".claude").join("settings.json")
    }

    fn manifest_path(&self) -> PathBuf {
        self.runtime_directory.join(".mcp-sync-manifest.json")
    }
}

fn make_catalog_id(scope: McpScope, workspace: Option<&str>, server_key: &str) -> String {
    match scope {
        McpScope::Global => format!("global::{server_key}"),
        McpScope::Project => {
            let workspace = workspace.unwrap_or_default();
            format!("project::{workspace}::{server_key}")
        }
    }
}

fn infer_scope_workspace_and_server(raw_catalog_id: &str) -> InferredCatalogId<'_> {
    if let Some(server_key) = raw_catalog_id.strip_prefix("global::") {
        return InferredCatalogId {
            scope: Some("global"),
            workspace: None,
            server_key: Some(server_key),
        };
    }

    if let Some(rest) = raw_catalog_id.strip_prefix("project::") {
        if let Some(split_index) = rest.rfind("::") {
            let workspace = &rest[..split_index];
            let server_key = &rest[split_index + 2..];
            if !workspace.is_empty() && !server_key.is_empty() {
                return InferredCatalogId {
                    scope: Some("project"),
                    workspace: Some(workspace),
                    server_key: Some(server_key),
                };
            }
        }
    }

    InferredCatalogId {
        scope: Some("global"),
        workspace: None,
        server_key: Some(raw_catalog_id),
    }
}

struct InferredCatalogId<'a> {
    scope: Option<&'a str>,
    workspace: Option<&'a str>,
    server_key: Option<&'a str>,
}

struct RemovalOutcome {
    removed_count: usize,
    changed_files: Vec<String>,
}

fn collect_broken_unmanaged_claude_candidates(
    discovered: &[Discovered],
    catalog: &BTreeMap<String, CatalogEntry>,
) -> Vec<BrokenUnmanagedClaudeCandidate> {
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    for item in discovered {
        if catalog.contains_key(&item.entry.catalog_id) {
            continue;
        }
        let Some(candidate) = build_broken_unmanaged_claude_candidate(item) else {
            continue;
        };
        let key = format!(
            "{}::{}::{}::{}",
            candidate.output.file_path,
            candidate.output.server_key,
            candidate.output.scope,
            candidate.output.workspace.clone().unwrap_or_default()
        );
        if seen.insert(key) {
            result.push(candidate);
        }
    }
    result.sort_by(|lhs, rhs| {
        lhs.output
            .file_path
            .cmp(&rhs.output.file_path)
            .then_with(|| lhs.output.scope.cmp(&rhs.output.scope))
            .then_with(|| lhs.output.workspace.cmp(&rhs.output.workspace))
            .then_with(|| lhs.output.server_key.cmp(&rhs.output.server_key))
            .then_with(|| lhs.output.reason.cmp(&rhs.output.reason))
    });
    result
}

fn format_broken_unmanaged_claude_warning(candidate: &UnmanagedClaudeMcpCandidate) -> String {
    format!(
        "Broken unmanaged Claude MCP '{}' in {}: {}",
        candidate.server_key, candidate.file_path, candidate.reason
    )
}

fn parse_warning_fix_action(warning: &str) -> Option<WarningFixAction> {
    parse_unmanaged_in_central_warning(warning)
        .or_else(|| parse_inline_secret_env_warning(warning))
        .or_else(|| parse_inline_secret_argument_warning(warning))
        .or_else(|| parse_skipped_managed_codex_warning(warning))
        .or_else(|| parse_skipped_missing_project_target_warning(warning))
}

fn parse_unmanaged_in_central_warning(warning: &str) -> Option<WarningFixAction> {
    let (server_key, rest) = warning.strip_prefix("MCP server '")?.split_once("' (")?;
    let (catalog_id, rest) = rest.split_once(") exists in ")?;
    let file_path = rest.strip_suffix(" but is unmanaged in central catalog")?;

    if server_key.trim().is_empty() || catalog_id.trim().is_empty() || file_path.trim().is_empty() {
        return None;
    }

    Some(WarningFixAction::UnmanagedInCentral {
        server_key: server_key.to_string(),
        catalog_id: catalog_id.to_string(),
        file_path: file_path.to_string(),
    })
}

fn parse_inline_secret_env_warning(warning: &str) -> Option<WarningFixAction> {
    let (server_key, rest) = warning
        .strip_prefix("MCP server '")?
        .split_once("' has inline secret-like env value for '")?;
    let env_key = rest.strip_suffix('\'')?;
    if server_key.trim().is_empty() || env_key.trim().is_empty() {
        return None;
    }

    Some(WarningFixAction::InlineSecretEnv {
        server_key: server_key.to_string(),
        env_key: env_key.to_string(),
    })
}

fn parse_skipped_managed_codex_warning(warning: &str) -> Option<WarningFixAction> {
    let (server_key, file_path) = warning
        .strip_prefix("Skipped managed Codex MCP '")?
        .split_once("' because unmanaged entry already exists in ")?;
    if server_key.trim().is_empty() || file_path.trim().is_empty() {
        return None;
    }

    Some(WarningFixAction::SkippedManagedCodex {
        server_key: server_key.to_string(),
        file_path: file_path.to_string(),
    })
}

fn parse_inline_secret_argument_warning(warning: &str) -> Option<WarningFixAction> {
    let (server_key, rest) = warning
        .strip_prefix("MCP server '")?
        .split_once("' has inline secret-like argument '")?;
    let redacted_argument = rest.strip_suffix('\'')?;
    if server_key.trim().is_empty() || redacted_argument.trim().is_empty() {
        return None;
    }
    Some(WarningFixAction::InlineSecretArgument {
        server_key: server_key.to_string(),
        redacted_argument: redacted_argument.to_string(),
    })
}

fn parse_skipped_missing_project_target_warning(warning: &str) -> Option<WarningFixAction> {
    let file_path = warning
        .strip_prefix("Skipped project MCP target ")?
        .strip_suffix(" because file does not exist")?;
    if file_path.trim().is_empty() {
        return None;
    }
    Some(WarningFixAction::SkippedMissingProjectTarget {
        file_path: file_path.to_string(),
    })
}

fn build_broken_unmanaged_claude_candidate(
    item: &Discovered,
) -> Option<BrokenUnmanagedClaudeCandidate> {
    if !is_claude_json_source(item.source) {
        return None;
    }
    let location = source_to_claude_location(item)?;
    let reason = detect_broken_unmanaged_claude_reason(&item.entry.definition)?;
    Some(BrokenUnmanagedClaudeCandidate {
        output: UnmanagedClaudeMcpCandidate {
            server_key: item.entry.server_key.clone(),
            scope: item.entry.scope.as_str().to_string(),
            workspace: item.entry.workspace.clone(),
            file_path: item.file_path.display().to_string(),
            reason,
        },
        path: item.file_path.clone(),
        location,
        server_key: item.entry.server_key.clone(),
    })
}

fn is_claude_json_source(source: SourceKind) -> bool {
    matches!(
        source,
        SourceKind::ClaudeUserGlobal
            | SourceKind::ClaudeLocalGlobal
            | SourceKind::ClaudeGlobalGlobal
            | SourceKind::ClaudeUserProject
    )
}

fn source_to_claude_location(item: &Discovered) -> Option<ClaudeJsonTargetLocation> {
    match item.source {
        SourceKind::ClaudeUserProject => Some(ClaudeJsonTargetLocation::Project {
            workspace: item.entry.workspace.clone()?,
        }),
        SourceKind::ClaudeUserGlobal
        | SourceKind::ClaudeLocalGlobal
        | SourceKind::ClaudeGlobalGlobal => Some(ClaudeJsonTargetLocation::Root),
        _ => None,
    }
}

fn detect_broken_unmanaged_claude_reason(definition: &McpDefinition) -> Option<String> {
    if definition.transport != McpTransport::Stdio {
        return None;
    }

    let command = definition.command.as_deref()?.trim();
    if command.is_empty() {
        return None;
    }

    let command_path = Path::new(command);
    if command_path.is_absolute() && !command_path.exists() {
        return Some(format!(
            "stdio command path does not exist: {}",
            command_path.display()
        ));
    }

    if is_interpreter_style_command(command) {
        let first_arg = definition.args.first().map(|value| value.trim())?;
        let first_arg_path = Path::new(first_arg);
        if first_arg_path.is_absolute() && !first_arg_path.exists() {
            return Some(format!(
                "stdio interpreter arg path does not exist: {}",
                first_arg_path.display()
            ));
        }
    }

    None
}

fn is_interpreter_style_command(command: &str) -> bool {
    let binary = Path::new(command)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(command)
        .to_ascii_lowercase();
    matches!(
        binary.as_str(),
        "node"
            | "node.exe"
            | "nodejs"
            | "python"
            | "python.exe"
            | "python3"
            | "python3.exe"
            | "bun"
            | "bun.exe"
            | "deno"
            | "deno.exe"
            | "uv"
            | "uvx"
            | "tsx"
    )
}

fn remove_server_keys(servers: &mut JsonMap<String, JsonValue>, keys: &BTreeSet<String>) -> usize {
    let mut removed = 0usize;
    for key in keys {
        if servers.remove(key).is_some() {
            removed += 1;
        }
    }
    removed
}

fn push_json_plan_entry(
    plans: &mut Vec<JsonWritePlan>,
    path: &Path,
    location: JsonTargetLocation,
    create_when_missing: bool,
    entry: JsonCatalogEntry,
) {
    for plan in plans.iter_mut() {
        if plan.path == path && plan.location == location {
            plan.create_when_missing = plan.create_when_missing || create_when_missing;
            plan.entries.push(entry);
            return;
        }
    }
    plans.push(JsonWritePlan {
        path: path.to_path_buf(),
        location,
        create_when_missing,
        entries: vec![entry],
    });
}

fn append_manifest_targets(
    targets: &mut BTreeMap<String, Vec<String>>,
    path: &str,
    values: Vec<String>,
) {
    let entry = targets.entry(path.to_string()).or_default();
    for value in values {
        if !entry.iter().any(|item| item == &value) {
            entry.push(value);
        }
    }
    entry.sort();
}

fn append_manifest_codex_enabled(
    codex_enabled: &mut BTreeMap<String, bool>,
    scope: McpScope,
    workspace: Option<&str>,
    entries: &BTreeMap<String, CodexCatalogEntry>,
    written_keys: &[String],
) {
    for key in written_keys {
        let Some(entry) = entries.get(key) else {
            continue;
        };
        let locator = make_catalog_id(scope, workspace, key);
        codex_enabled.insert(locator, entry.enabled);
    }
}

fn locator_for_entry(entry: &CatalogEntry) -> String {
    match entry.scope {
        McpScope::Global => format!("global::{}", entry.server_key),
        McpScope::Project => {
            let workspace = entry.workspace.as_deref().unwrap_or_default();
            format!("project::{workspace}::{}", entry.server_key)
        }
    }
}

fn locator_server_key(locator: &str) -> Option<&str> {
    if let Some(rest) = locator.strip_prefix("global::") {
        return Some(rest);
    }
    if let Some(rest) = locator.strip_prefix("project::") {
        if let Some(index) = rest.rfind("::") {
            return Some(&rest[index + 2..]);
        }
    }
    None
}

fn is_global_locator(locator: &str) -> bool {
    locator.starts_with("global::")
}

fn project_locator_workspace(locator: &str) -> Option<&str> {
    let rest = locator.strip_prefix("project::")?;
    let index = rest.rfind("::")?;
    Some(&rest[..index])
}

fn filter_manifest_locators_for_location(
    path: &Path,
    location: &JsonTargetLocation,
    locators: &[String],
) -> Vec<String> {
    let mut result = Vec::new();
    for locator in locators {
        if is_global_locator(locator) {
            if *location == JsonTargetLocation::Root {
                result.push(locator.clone());
            }
            continue;
        }
        if let Some(workspace) = project_locator_workspace(locator) {
            if let JsonTargetLocation::Project {
                workspace: expected,
            } = location
            {
                if workspace == expected {
                    result.push(locator.clone());
                }
            }
            continue;
        }
        if *location == JsonTargetLocation::Root && !path.ends_with(".claude.json") {
            result.push(locator.clone());
        }
    }
    result
}

fn group_locators_by_location(
    path: &Path,
    locators: &[String],
) -> BTreeMap<JsonTargetLocation, Vec<String>> {
    let mut groups = BTreeMap::new();
    for locator in locators {
        if is_global_locator(locator) {
            groups
                .entry(JsonTargetLocation::Root)
                .or_insert_with(Vec::new)
                .push(locator.clone());
            continue;
        }
        if let Some(workspace) = project_locator_workspace(locator) {
            groups
                .entry(JsonTargetLocation::Project {
                    workspace: workspace.to_string(),
                })
                .or_insert_with(Vec::new)
                .push(locator.clone());
            continue;
        }
        groups
            .entry(JsonTargetLocation::Root)
            .or_insert_with(Vec::new)
            .push(locator.clone());
    }

    if groups.is_empty() && path.exists() {
        groups.insert(JsonTargetLocation::Root, Vec::new());
    }

    groups
}

fn json_location_key(path: &str, location: &JsonTargetLocation) -> String {
    match location {
        JsonTargetLocation::Root => format!("{path}::root"),
        JsonTargetLocation::Project { workspace } => {
            format!("{path}::project::{workspace}")
        }
    }
}

fn was_previously_managed_claude_locator(manifest: &McpManifest, locator: &str) -> bool {
    let server_key = locator_server_key(locator).unwrap_or(locator);
    for (path, items) in &manifest.targets {
        if !path.ends_with(".json") {
            continue;
        }
        for item in items {
            if item == locator {
                return true;
            }
            if locator_server_key(item).is_none() && item == server_key {
                return true;
            }
        }
    }
    false
}

fn codex_entries_for_catalog(entries: &[CatalogEntry]) -> BTreeMap<String, CodexCatalogEntry> {
    let mut definitions = BTreeMap::new();
    for item in entries {
        definitions.insert(
            item.server_key.clone(),
            CodexCatalogEntry {
                definition: item.definition.clone(),
                enabled: item.enabled_by_agent.codex,
            },
        );
    }
    definitions
}

fn definition_to_json(definition: &McpDefinition) -> JsonValue {
    let mut object = JsonMap::new();
    match definition.transport {
        McpTransport::Http => {
            object.insert("type".to_string(), JsonValue::String("http".to_string()));
            if let Some(url) = &definition.url {
                object.insert("url".to_string(), JsonValue::String(url.clone()));
            }
        }
        McpTransport::Stdio => {
            object.insert("type".to_string(), JsonValue::String("stdio".to_string()));
            if let Some(command) = &definition.command {
                object.insert("command".to_string(), JsonValue::String(command.clone()));
            }
            if !definition.args.is_empty() {
                object.insert(
                    "args".to_string(),
                    JsonValue::Array(
                        definition
                            .args
                            .iter()
                            .map(|item| JsonValue::String(item.clone()))
                            .collect(),
                    ),
                );
            }
        }
    }

    if !definition.env.is_empty() {
        let mut env = JsonMap::new();
        for (key, value) in &definition.env {
            env.insert(key.clone(), JsonValue::String(value.clone()));
        }
        object.insert("env".to_string(), JsonValue::Object(env));
    }

    JsonValue::Object(object)
}

fn parse_json_server_map(map: &JsonMap<String, JsonValue>) -> Vec<(String, McpDefinition, bool)> {
    let mut result = Vec::new();
    for (server_key, value) in map {
        let Some(obj) = value.as_object() else {
            continue;
        };
        let transport = match obj.get("type").and_then(JsonValue::as_str) {
            Some("http") => McpTransport::Http,
            _ => {
                if obj.get("url").is_some() {
                    McpTransport::Http
                } else {
                    McpTransport::Stdio
                }
            }
        };
        let command = obj
            .get("command")
            .and_then(JsonValue::as_str)
            .map(ToString::to_string);
        let args = obj
            .get("args")
            .and_then(JsonValue::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(JsonValue::as_str)
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let url = obj
            .get("url")
            .and_then(JsonValue::as_str)
            .map(ToString::to_string);
        let enabled = obj
            .get("enabled")
            .and_then(JsonValue::as_bool)
            .unwrap_or(true);

        let mut env = BTreeMap::new();
        if let Some(env_obj) = obj.get("env").and_then(JsonValue::as_object) {
            for (key, value) in env_obj {
                if let Some(value) = value.as_str() {
                    env.insert(key.clone(), value.to_string());
                }
            }
        }

        result.push((
            server_key.clone(),
            McpDefinition {
                transport,
                command,
                args,
                url,
                env,
            },
            enabled,
        ));
    }
    result
}

fn read_json_servers(path: &Path) -> Result<Vec<(String, McpDefinition, bool)>, String> {
    let raw = fs::read_to_string(path).map_err(|error| error.to_string())?;
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }

    let parsed = serde_json::from_str::<JsonValue>(&raw).map_err(|error| error.to_string())?;
    let Some(root) = parsed.as_object() else {
        return Err("root must be object".to_string());
    };

    let Some(mcp) = root.get("mcpServers") else {
        return Ok(Vec::new());
    };
    let Some(map) = mcp.as_object() else {
        return Err("mcpServers must be object".to_string());
    };

    Ok(parse_json_server_map(map))
}

struct ClaudeUserServers {
    global: Vec<(String, McpDefinition, bool)>,
    projects: BTreeMap<String, Vec<(String, McpDefinition, bool)>>,
}

fn read_claude_user_servers(path: &Path) -> Result<ClaudeUserServers, String> {
    let raw = fs::read_to_string(path).map_err(|error| error.to_string())?;
    if raw.trim().is_empty() {
        return Ok(ClaudeUserServers {
            global: Vec::new(),
            projects: BTreeMap::new(),
        });
    }

    let parsed = serde_json::from_str::<JsonValue>(&raw).map_err(|error| error.to_string())?;
    let Some(root) = parsed.as_object() else {
        return Err("root must be object".to_string());
    };

    let global = match root.get("mcpServers") {
        Some(value) => match value.as_object() {
            Some(map) => parse_json_server_map(map),
            None => return Err("mcpServers must be object".to_string()),
        },
        None => Vec::new(),
    };

    let mut projects = BTreeMap::new();
    if let Some(projects_value) = root.get("projects") {
        let Some(projects_obj) = projects_value.as_object() else {
            return Err("projects must be object".to_string());
        };
        for (workspace, value) in projects_obj {
            let Some(project_obj) = value.as_object() else {
                continue;
            };
            let Some(project_servers) = project_obj.get("mcpServers") else {
                continue;
            };
            let Some(server_map) = project_servers.as_object() else {
                return Err(format!("projects.{workspace}.mcpServers must be object"));
            };
            projects.insert(workspace.clone(), parse_json_server_map(server_map));
        }
    }

    Ok(ClaudeUserServers { global, projects })
}

fn read_toml_servers(path: &Path) -> Result<Vec<(String, McpDefinition, bool)>, String> {
    let raw = fs::read_to_string(path).map_err(|error| error.to_string())?;
    read_toml_servers_from_str(&raw)
}

fn read_toml_servers_from_str(raw: &str) -> Result<Vec<(String, McpDefinition, bool)>, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(Vec::new());
    }

    let parsed: toml::Table = toml::from_str(raw).map_err(|error| error.to_string())?;
    let Some(root) = parsed.get("mcp_servers") else {
        return Ok(Vec::new());
    };
    let Some(table) = root.as_table() else {
        return Ok(Vec::new());
    };

    let mut result = Vec::new();
    for (server_key, value) in table {
        let Some(server_table) = value.as_table() else {
            continue;
        };

        let definition = definition_from_toml_table(server_table);
        let enabled = server_table
            .get("enabled")
            .and_then(toml::Value::as_bool)
            .unwrap_or(true);
        result.push((server_key.to_string(), definition, enabled));
    }

    Ok(result)
}

/// Returns all text forms of a TOML section header for `[mcp_servers.{key}]`,
/// including both quoted and unquoted variants.
fn codex_server_headers(key: &str) -> Vec<String> {
    vec![
        format!("[mcp_servers.{}]", key),
        format!("[mcp_servers.\"{}\"]", key),
        format!("[mcp_servers.'{}']", key),
    ]
}

/// Returns all text prefixes for sub-sections of `[mcp_servers.{key}.*]`.
fn codex_server_sub_prefixes(key: &str) -> Vec<String> {
    vec![
        format!("[mcp_servers.{}.", key),
        format!("[mcp_servers.\"{}\".", key),
        format!("[mcp_servers.'{}\'.", key),
    ]
}

/// Checks if `[mcp_servers.{key}]` header exists in raw TOML text.
/// Used as a fallback when structured TOML parsing fails.
fn text_contains_codex_server(text: &str, key: &str) -> bool {
    let headers = codex_server_headers(key);
    text.lines()
        .any(|line| headers.iter().any(|h| line.trim() == *h))
}

/// Removes `[mcp_servers.{key}]` section (and its sub-sections like `.env`)
/// from raw TOML text, preserving all comments and other content.
fn text_remove_codex_server_section(text: &str, key: &str) -> String {
    let headers = codex_server_headers(key);
    let sub_prefixes = codex_server_sub_prefixes(key);
    let mut result = Vec::new();
    let mut skipping = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if skipping {
            // Check if this line is a new section header that is NOT a sub-section
            if trimmed.starts_with('[')
                && !sub_prefixes.iter().any(|p| trimmed.starts_with(p.as_str()))
            {
                skipping = false;
                result.push(line);
            }
            // else: skip this line (part of removed section or sub-section)
        } else if headers.iter().any(|h| trimmed == *h)
            || sub_prefixes.iter().any(|p| trimmed.starts_with(p.as_str()))
        {
            skipping = true;
            // skip this line
        } else {
            result.push(line);
        }
    }

    // Collapse runs of >2 blank lines into at most 1 blank line
    let mut collapsed = Vec::new();
    let mut blank_run = 0;
    for line in &result {
        if line.trim().is_empty() {
            blank_run += 1;
            if blank_run <= 1 {
                collapsed.push(*line);
            }
        } else {
            blank_run = 0;
            collapsed.push(*line);
        }
    }

    // Trim leading/trailing blank lines
    while collapsed.first().is_some_and(|l| l.trim().is_empty()) {
        collapsed.remove(0);
    }
    while collapsed.last().is_some_and(|l| l.trim().is_empty()) {
        collapsed.pop();
    }

    let mut out = collapsed.join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

fn parse_unmanaged_codex_table(
    unmanaged: &str,
    path: &Path,
    warnings: &mut Vec<String>,
) -> Option<toml::Table> {
    if unmanaged.trim().is_empty() {
        return Some(toml::Table::new());
    }
    match toml::from_str::<toml::Table>(unmanaged) {
        Ok(table) => Some(table),
        Err(error) => {
            warnings.push(format!(
                "Failed to parse unmanaged Codex MCP block {}: {error}",
                path.display()
            ));
            None
        }
    }
}

fn unmanaged_codex_contains_server(table: &toml::Table, server_key: &str) -> bool {
    table
        .get("mcp_servers")
        .and_then(toml::Value::as_table)
        .and_then(|servers| servers.get(server_key))
        .is_some()
}

fn definition_from_toml_table(table: &toml::value::Table) -> McpDefinition {
    let command = table
        .get("command")
        .and_then(toml::Value::as_str)
        .map(ToString::to_string);
    let args = table
        .get("args")
        .and_then(toml::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(toml::Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let url = table
        .get("url")
        .and_then(toml::Value::as_str)
        .map(ToString::to_string);

    let transport = match table
        .get("type")
        .and_then(toml::Value::as_str)
        .unwrap_or("")
    {
        "http" => McpTransport::Http,
        _ => {
            if url.is_some() {
                McpTransport::Http
            } else {
                McpTransport::Stdio
            }
        }
    };

    let mut env = BTreeMap::new();
    if let Some(env_table) = table.get("env").and_then(toml::Value::as_table) {
        for (key, value) in env_table {
            if let Some(value) = value.as_str() {
                env.insert(key.to_string(), value.to_string());
            }
        }
    }

    McpDefinition {
        transport,
        command,
        args,
        url,
        env,
    }
}

fn enabled_from_toml_table(table: &toml::value::Table) -> McpEnabledByAgent {
    let mut result = McpEnabledByAgent::default();
    if let Some(enabled) = table
        .get("enabled_by_agent")
        .and_then(toml::Value::as_table)
    {
        if let Some(value) = enabled.get("codex").and_then(toml::Value::as_bool) {
            result.codex = value;
        }
        if let Some(value) = enabled.get("claude").and_then(toml::Value::as_bool) {
            result.claude = value;
        }
        if let Some(value) = enabled.get("project").and_then(toml::Value::as_bool) {
            result.project = value;
        }
    }
    result
}

fn build_targets(
    codex_path: &Path,
    global_claude_path: &Path,
    claude_user_path: &Path,
    entry: &CatalogEntry,
) -> Vec<String> {
    let mut targets = Vec::new();
    match entry.scope {
        McpScope::Global => {
            if entry.enabled_by_agent.codex {
                targets.push(codex_path.display().to_string());
            }
            if entry.enabled_by_agent.claude {
                targets.push(global_claude_path.display().to_string());
            }
        }
        McpScope::Project => {
            let Some(workspace) = entry.workspace.as_ref() else {
                return targets;
            };
            let workspace_path = PathBuf::from(workspace);
            let project_codex = workspace_path.join(".codex").join("config.toml");
            let project_mcp = workspace_path.join(".mcp.json");
            if entry.enabled_by_agent.project
                && entry.enabled_by_agent.codex
                && project_codex.exists()
            {
                targets.push(project_codex.display().to_string());
            }
            if entry.enabled_by_agent.project && entry.enabled_by_agent.claude {
                match entry.project_claude_target {
                    ProjectClaudeTarget::WorkspaceMcpJson => {
                        if project_mcp.exists() {
                            targets.push(project_mcp.display().to_string());
                        }
                    }
                    ProjectClaudeTarget::ClaudeUserProject => {
                        targets.push(claude_user_path.display().to_string());
                    }
                }
            }
        }
    }
    targets.sort();
    targets.dedup();
    targets
}

fn render_central_block(catalog: &BTreeMap<String, CatalogEntry>) -> String {
    if catalog.is_empty() {
        return "# No managed MCP entries".to_string();
    }

    let mut mcp_catalog = toml::Table::new();
    for (catalog_id, entry) in catalog {
        let mut table = toml::Table::new();
        table.insert(
            "server_key".into(),
            toml::Value::String(entry.server_key.clone()),
        );
        table.insert(
            "scope".into(),
            toml::Value::String(entry.scope.as_str().to_string()),
        );
        table.insert(
            "status".into(),
            toml::Value::String(
                match entry.status {
                    SkillLifecycleStatus::Active => "active",
                    SkillLifecycleStatus::Archived => "archived",
                }
                .to_string(),
            ),
        );
        if let Some(archived_at) = &entry.archived_at {
            table.insert(
                "archived_at".into(),
                toml::Value::String(archived_at.clone()),
            );
        }
        if let Some(workspace) = &entry.workspace {
            table.insert("workspace".into(), toml::Value::String(workspace.clone()));
            table.insert(
                "project_claude_target".into(),
                toml::Value::String(entry.project_claude_target.as_str().to_string()),
            );
        }
        table.insert(
            "transport".into(),
            toml::Value::String(
                match entry.definition.transport {
                    McpTransport::Stdio => "stdio",
                    McpTransport::Http => "http",
                }
                .to_string(),
            ),
        );
        if let Some(command) = &entry.definition.command {
            table.insert("command".into(), toml::Value::String(command.clone()));
        }
        if !entry.definition.args.is_empty() {
            table.insert(
                "args".into(),
                toml::Value::Array(
                    entry
                        .definition
                        .args
                        .iter()
                        .map(|a| toml::Value::String(a.clone()))
                        .collect(),
                ),
            );
        }
        if let Some(url) = &entry.definition.url {
            table.insert("url".into(), toml::Value::String(url.clone()));
        }
        if !entry.definition.env.is_empty() {
            let mut env_table = toml::Table::new();
            for (env_key, env_value) in &entry.definition.env {
                env_table.insert(env_key.clone(), toml::Value::String(env_value.clone()));
            }
            table.insert("env".into(), toml::Value::Table(env_table));
        }
        let mut enabled_table = toml::Table::new();
        enabled_table.insert(
            "codex".into(),
            toml::Value::Boolean(entry.enabled_by_agent.codex),
        );
        enabled_table.insert(
            "claude".into(),
            toml::Value::Boolean(entry.enabled_by_agent.claude),
        );
        enabled_table.insert(
            "project".into(),
            toml::Value::Boolean(entry.enabled_by_agent.project),
        );
        table.insert("enabled_by_agent".into(), toml::Value::Table(enabled_table));
        mcp_catalog.insert(catalog_id.clone(), toml::Value::Table(table));
    }

    let mut root = toml::Table::new();
    root.insert("mcp_catalog".into(), toml::Value::Table(mcp_catalog));
    toml::to_string(&root)
        .expect("BUG: invalid TOML table")
        .trim_end()
        .to_string()
}

fn render_codex_block(entries: &BTreeMap<String, CodexCatalogEntry>) -> String {
    if entries.is_empty() {
        return "# No managed MCP entries".to_string();
    }

    let mut servers = toml::Table::new();
    for (key, item) in entries {
        let definition = &item.definition;
        let mut entry = toml::Table::new();
        if let Some(command) = &definition.command {
            entry.insert("command".into(), toml::Value::String(command.clone()));
        }
        if !definition.args.is_empty() {
            entry.insert(
                "args".into(),
                toml::Value::Array(
                    definition
                        .args
                        .iter()
                        .map(|a| toml::Value::String(a.clone()))
                        .collect(),
                ),
            );
        }
        if let Some(url) = &definition.url {
            entry.insert("url".into(), toml::Value::String(url.clone()));
        }
        entry.insert("enabled".into(), toml::Value::Boolean(item.enabled));
        if !definition.env.is_empty() {
            let mut env_table = toml::Table::new();
            for (env_key, env_value) in &definition.env {
                env_table.insert(env_key.clone(), toml::Value::String(env_value.clone()));
            }
            entry.insert("env".into(), toml::Value::Table(env_table));
        }
        servers.insert(key.clone(), toml::Value::Table(entry));
    }

    let mut root = toml::Table::new();
    root.insert("mcp_servers".into(), toml::Value::Table(servers));
    toml::to_string(&root)
        .expect("BUG: invalid TOML table")
        .trim_end()
        .to_string()
}

fn detect_inline_secret_warnings(server_key: &str, definition: &McpDefinition) -> Vec<String> {
    let mut warnings = Vec::new();
    for (key, value) in &definition.env {
        if !is_secret_like_key(key) {
            continue;
        }
        if !value.starts_with("${") {
            warnings.push(format!(
                "MCP server '{}' has inline secret-like env value for '{}'",
                server_key, key
            ));
        }
    }
    for arg in &definition.args {
        if let Some(redacted) = redact_secret_like_arg(arg) {
            warnings.push(format!(
                "MCP server '{}' has inline secret-like argument '{}'",
                server_key, redacted
            ));
        }
    }
    warnings
}

fn is_non_empty_env_var(key: &str) -> bool {
    std::env::var(key)
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn is_secret_like_key(key: &str) -> bool {
    let normalized = key.trim().to_ascii_lowercase();
    normalized.contains("token")
        || normalized.contains("secret")
        || normalized.contains("password")
        || normalized.contains("api_key")
        || normalized.contains("apikey")
        || normalized.contains("license")
        || normalized.contains("email")
}

fn redact_secret_like_arg(arg: &str) -> Option<String> {
    let (key, value) = arg.split_once('=')?;
    if value.is_empty() || value.contains("${") {
        return None;
    }
    if !is_secret_like_key(key) {
        return None;
    }
    Some(format!("{key}=<redacted>"))
}

fn secret_arg_env_key(arg_key: &str) -> String {
    let trimmed = arg_key.trim_start_matches('-');
    let mut value = String::new();
    let mut last_underscore = false;

    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() {
            value.push(ch.to_ascii_uppercase());
            last_underscore = false;
        } else if !last_underscore {
            value.push('_');
            last_underscore = true;
        }
    }

    let normalized = value.trim_matches('_').to_string();
    if normalized.is_empty() {
        String::from("SECRET")
    } else {
        normalized
    }
}

fn source_label(source: SourceKind) -> &'static str {
    match source {
        SourceKind::CodexGlobal => "codex-global",
        SourceKind::ClaudeUserGlobal => "claude-user-global",
        SourceKind::ClaudeLocalGlobal => "claude-local-global",
        SourceKind::ClaudeGlobalGlobal => "claude-global-global",
        SourceKind::ClaudeUserProject => "claude-user-project",
        SourceKind::ProjectCodex => "project-codex",
        SourceKind::ProjectClaude => "project-claude",
    }
}

fn source_priority(source: SourceKind) -> u8 {
    match source {
        SourceKind::ProjectClaude => 0,
        SourceKind::ClaudeUserProject => 1,
        SourceKind::ProjectCodex => 2,
        SourceKind::CodexGlobal => 10,
        SourceKind::ClaudeUserGlobal => 11,
        SourceKind::ClaudeLocalGlobal => 12,
        SourceKind::ClaudeGlobalGlobal => 13,
    }
}

fn extract_managed_block(current: &str, begin_marker: &str, end_marker: &str) -> Option<String> {
    let normalized = current.replace("\r\n", "\n");
    let begin_index = normalized.find(begin_marker)?;
    let end_index = normalized[begin_index..].find(end_marker)?;
    let body_start = begin_index + begin_marker.len();
    let body_end = begin_index + end_index;
    Some(
        normalized[body_start..body_end]
            .trim_matches('\n')
            .to_string(),
    )
}

fn extract_managed_block_from_markers(
    current: &str,
    marker_pairs: &[(&str, &str)],
) -> Option<String> {
    let mut fallback = None;
    for &(begin_marker, end_marker) in marker_pairs {
        if let Some(block) = extract_managed_block(current, begin_marker, end_marker) {
            if fallback.is_none() {
                fallback = Some(block.clone());
            }
            if !is_effectively_empty_managed_block(&block) {
                return Some(block);
            }
        }
    }
    fallback
}

fn is_effectively_empty_managed_block(body: &str) -> bool {
    body.lines().all(|line| {
        let trimmed = line.trim();
        trimmed.is_empty() || trimmed.starts_with('#')
    })
}

fn iso8601_now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::{
        extract_managed_block_from_markers, is_secret_like_key, read_json_servers,
        read_toml_servers_from_str, render_central_block, render_codex_block,
        text_contains_codex_server, text_remove_codex_server_section, CatalogEntry,
        CodexCatalogEntry, McpDefinition, McpRegistry, McpScope, ProjectClaudeTarget,
        CENTRAL_BEGIN, CENTRAL_END, CENTRAL_MARKER_PAIRS, CODEX_MARKER_PAIRS,
    };
    use crate::managed_block::{strip_managed_blocks, upsert_managed_block};
    use crate::models::{
        CatalogMutationAction, McpEnabledByAgent, McpTransport, SkillLifecycleStatus,
    };
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    fn registry_in_temp() -> (tempfile::TempDir, McpRegistry, PathBuf, PathBuf) {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("home");
        let runtime = temp.path().join("runtime");
        std::fs::create_dir_all(&home).expect("home");
        std::fs::create_dir_all(&runtime).expect("runtime");
        let registry = McpRegistry::new(home.clone(), runtime.clone());
        (temp, registry, home, runtime)
    }

    fn write_central_catalog(home: &Path, catalog: &BTreeMap<String, CatalogEntry>) {
        let config_path = home.join(".config").join("ai-agents").join("config.toml");
        let wrapped = format!(
            "{CENTRAL_BEGIN}\n{}\n{CENTRAL_END}\n",
            render_central_block(catalog)
        );
        std::fs::create_dir_all(config_path.parent().expect("parent")).expect("mkdir parent");
        std::fs::write(&config_path, wrapped).expect("write central catalog");
    }

    fn count_occurrences(body: &str, needle: &str) -> usize {
        body.match_indices(needle).count()
    }

    #[test]
    fn upsert_managed_block_replaces_existing() {
        let current = "alpha = true\n\n# agent-sync:mcp:begin\nold = true\n# agent-sync:mcp:end\n";
        let next = upsert_managed_block(
            current,
            "# agent-sync:mcp:begin",
            "# agent-sync:mcp:end",
            "new = true",
        );
        assert!(next.contains("new = true"));
        assert!(!next.contains("old = true"));
        assert!(next.contains("alpha = true"));
    }

    #[test]
    fn strip_managed_blocks_removes_legacy_and_current_blocks() {
        let current = "\
before = true

# skills-sync:mcp:codex:begin
[mcp_servers.old]
command = \"old\"
# skills-sync:mcp:codex:end

# agent-sync:mcp:codex:begin
[mcp_servers.new]
command = \"new\"
# agent-sync:mcp:codex:end
";

        let stripped = strip_managed_blocks(current, &CODEX_MARKER_PAIRS);
        assert_eq!(stripped.trim(), "before = true");
    }

    #[test]
    fn extract_managed_block_prefers_non_empty_legacy_when_current_is_placeholder() {
        let current = "\
# agent-sync:mcp:begin
# No managed MCP entries
# agent-sync:mcp:end

# skills-sync:mcp:begin
[mcp_catalog.\"global::legacy\"]
server_key = \"legacy\"
scope = \"global\"
transport = \"stdio\"
command = \"legacy\"
[mcp_catalog.\"global::legacy\".enabled_by_agent]
codex = true
claude = true
project = false
# skills-sync:mcp:end
";

        let block = extract_managed_block_from_markers(current, &CENTRAL_MARKER_PAIRS)
            .expect("extract managed block");
        assert!(block.contains("[mcp_catalog.\"global::legacy\"]"));
    }

    #[test]
    fn read_toml_servers_supports_nested_env() {
        let raw = r#"
[mcp_servers.test]
command = "npx"
args = ["-y", "foo"]
enabled = false
[mcp_servers.test.env]
API_KEY = "${API_KEY}"
"#;
        let items = read_toml_servers_from_str(raw).expect("parse toml");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].0, "test");
        assert!(!items[0].2);
        assert_eq!(items[0].1.command.as_deref(), Some("npx"));
        assert_eq!(items[0].1.args, vec!["-y", "foo"]);
        assert_eq!(
            items[0].1.env.get("API_KEY").map(String::as_str),
            Some("${API_KEY}")
        );
    }

    #[test]
    fn render_codex_block_respects_enabled_flag() {
        let mut entries = BTreeMap::new();
        entries.insert(
            String::from("exa"),
            CodexCatalogEntry {
                definition: McpDefinition {
                    transport: McpTransport::Stdio,
                    command: Some(String::from("npx")),
                    args: vec![String::from("-y"), String::from("mcp-remote@latest")],
                    url: None,
                    env: BTreeMap::new(),
                },
                enabled: false,
            },
        );
        let rendered = render_codex_block(&entries);
        assert!(rendered.contains("[mcp_servers.exa]"));
        assert!(rendered.contains("enabled = false"));
    }

    #[test]
    fn apply_codex_catalog_path_auto_cleans_unmanaged_for_disabled_entry() {
        let (_temp, registry, home, _runtime) = registry_in_temp();
        let codex_path = home.join(".codex").join("config.toml");
        std::fs::create_dir_all(codex_path.parent().expect("parent")).expect("mkdir parent");
        std::fs::write(
            &codex_path,
            "# My custom comment\n\n\
             [mcp_servers.exa]\n\
             command = \"npx\"\n\
             args = [\"-y\", \"mcp-remote@latest\", \"https://mcp.exa.ai/mcp\"]\n\
             enabled = true\n\n\
             [mcp_servers.other]\n\
             command = \"other-cmd\"\n",
        )
        .expect("write codex");

        let mut entries = BTreeMap::new();
        entries.insert(
            String::from("exa"),
            CodexCatalogEntry {
                definition: McpDefinition {
                    transport: McpTransport::Stdio,
                    command: Some(String::from("npx")),
                    args: vec![
                        String::from("-y"),
                        String::from("mcp-remote@latest"),
                        String::from("https://mcp.exa.ai/mcp"),
                    ],
                    url: None,
                    env: BTreeMap::new(),
                },
                enabled: false,
            },
        );

        let mut warnings = Vec::new();
        let keys = registry
            .apply_codex_catalog_path(&codex_path, &entries, true, &mut warnings)
            .expect("apply codex");
        assert_eq!(keys, vec![String::from("exa")]);
        assert!(warnings
            .iter()
            .any(|item| item.contains("Auto-cleaned unmanaged Codex MCP 'exa'")));

        let codex_raw = std::fs::read_to_string(&codex_path).expect("read codex");
        assert_eq!(count_occurrences(&codex_raw, "[mcp_servers.exa]"), 1);
        assert!(codex_raw.contains("enabled = false"));
        // Comments and other unmanaged entries must be preserved
        assert!(
            codex_raw.contains("# My custom comment"),
            "comments should be preserved"
        );
        assert!(
            codex_raw.contains("[mcp_servers.other]"),
            "other unmanaged entries should be preserved"
        );
    }

    #[test]
    fn apply_codex_catalog_path_text_fallback_detects_duplicate_on_parse_failure() {
        let (_temp, registry, home, _runtime) = registry_in_temp();
        let codex_path = home.join(".codex").join("config.toml");
        std::fs::create_dir_all(codex_path.parent().expect("parent")).expect("mkdir parent");
        // Write intentionally unparseable TOML with a duplicate server key
        std::fs::write(
            &codex_path,
            "!!! invalid toml syntax !!!\n\n\
             [mcp_servers.exa]\n\
             command = \"npx\"\n\
             args = [\"-y\", \"mcp-remote@latest\"]\n",
        )
        .expect("write codex");

        let mut entries = BTreeMap::new();
        entries.insert(
            String::from("exa"),
            CodexCatalogEntry {
                definition: McpDefinition {
                    transport: McpTransport::Stdio,
                    command: Some(String::from("npx")),
                    args: vec![String::from("-y"), String::from("mcp-remote@latest")],
                    url: None,
                    env: BTreeMap::new(),
                },
                enabled: true,
            },
        );

        let mut warnings = Vec::new();
        registry
            .apply_codex_catalog_path(&codex_path, &entries, true, &mut warnings)
            .expect("apply codex");

        // Text fallback should detect the duplicate and skip managed entry
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("Skipped managed Codex MCP 'exa'")),
            "should skip duplicate via text fallback: {warnings:?}"
        );
    }

    #[test]
    fn text_remove_codex_server_section_removes_with_subtables() {
        let input = "\
[mcp_servers.foo]\n\
command = \"foo-cmd\"\n\
\n\
[mcp_servers.foo.env]\n\
API_KEY = \"secret\"\n\
\n\
[mcp_servers.bar]\n\
command = \"bar-cmd\"\n";

        let result = text_remove_codex_server_section(input, "foo");
        assert!(
            !result.contains("[mcp_servers.foo]"),
            "foo section should be removed"
        );
        assert!(
            !result.contains("[mcp_servers.foo.env]"),
            "foo.env sub-section should be removed"
        );
        assert!(
            result.contains("[mcp_servers.bar]"),
            "bar section should be preserved"
        );
        assert!(
            result.contains("bar-cmd"),
            "bar content should be preserved"
        );
    }

    #[test]
    fn text_contains_codex_server_matches_correctly() {
        let text = "\
[mcp_servers.exa]\n\
command = \"npx\"\n\
\n\
[mcp_servers.other]\n\
command = \"other\"\n";

        assert!(text_contains_codex_server(text, "exa"));
        assert!(text_contains_codex_server(text, "other"));
        assert!(!text_contains_codex_server(text, "missing"));
        // Should not match partial key names
        assert!(!text_contains_codex_server(text, "ex"));
        assert!(!text_contains_codex_server(text, "exa_extended"));
    }

    #[test]
    fn read_json_servers_supports_http() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("settings.local.json");
        std::fs::write(
            &path,
            r#"{
  "mcpServers": {
    "exa": {
      "type": "http",
      "url": "https://mcp.exa.ai/mcp",
      "enabled": true
    }
  }
}
"#,
        )
        .expect("write json");
        let items = read_json_servers(&path).expect("parse json");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].0, "exa");
        assert_eq!(items[0].1.url.as_deref(), Some("https://mcp.exa.ai/mcp"));
        assert!(items[0].2);
    }

    #[test]
    fn central_catalog_roundtrip_preserves_status_and_archived_at() {
        let (_temp, registry, home, _runtime) = registry_in_temp();
        let archived_at = "2026-03-02T12:34:56.000Z";

        let mut catalog = BTreeMap::new();
        catalog.insert(
            String::from("global::exa"),
            CatalogEntry {
                catalog_id: String::from("global::exa"),
                server_key: String::from("exa"),
                scope: McpScope::Global,
                workspace: None,
                definition: McpDefinition {
                    transport: McpTransport::Stdio,
                    command: Some(String::from("npx")),
                    args: vec![String::from("-y"), String::from("exa-mcp")],
                    url: None,
                    env: BTreeMap::new(),
                },
                enabled_by_agent: McpEnabledByAgent {
                    codex: true,
                    claude: false,
                    project: false,
                },
                project_claude_target: ProjectClaudeTarget::WorkspaceMcpJson,
                status: SkillLifecycleStatus::Archived,
                archived_at: Some(archived_at.to_string()),
            },
        );

        let block = render_central_block(&catalog);
        assert!(block.contains("status = \"archived\""));
        assert!(block.contains(&format!("archived_at = \"{archived_at}\"")));

        let config_path = home.join(".config").join("ai-agents").join("config.toml");
        let wrapped = format!("{CENTRAL_BEGIN}\n{block}\n{CENTRAL_END}\n");
        std::fs::create_dir_all(config_path.parent().expect("parent")).expect("mkdir parent");
        std::fs::write(&config_path, wrapped).expect("write central");

        let parsed = registry
            .load_central_catalog(&mut Vec::new())
            .expect("load central catalog");
        let entry = parsed.get("global::exa").expect("parsed entry");
        assert_eq!(entry.status, SkillLifecycleStatus::Archived);
        assert_eq!(entry.archived_at.as_deref(), Some(archived_at));
    }

    #[test]
    fn sync_writes_only_active_entries_but_returns_archived_records() {
        let (_temp, registry, home, _runtime) = registry_in_temp();

        let mut catalog = BTreeMap::new();
        catalog.insert(
            String::from("global::active-exa"),
            CatalogEntry {
                catalog_id: String::from("global::active-exa"),
                server_key: String::from("active-exa"),
                scope: McpScope::Global,
                workspace: None,
                definition: McpDefinition {
                    transport: McpTransport::Stdio,
                    command: Some(String::from("npx")),
                    args: vec![String::from("-y"), String::from("active-mcp")],
                    url: None,
                    env: BTreeMap::new(),
                },
                enabled_by_agent: McpEnabledByAgent {
                    codex: true,
                    claude: false,
                    project: false,
                },
                project_claude_target: ProjectClaudeTarget::WorkspaceMcpJson,
                status: SkillLifecycleStatus::Active,
                archived_at: None,
            },
        );
        catalog.insert(
            String::from("global::archived-exa"),
            CatalogEntry {
                catalog_id: String::from("global::archived-exa"),
                server_key: String::from("archived-exa"),
                scope: McpScope::Global,
                workspace: None,
                definition: McpDefinition {
                    transport: McpTransport::Stdio,
                    command: Some(String::from("npx")),
                    args: vec![String::from("-y"), String::from("archived-mcp")],
                    url: None,
                    env: BTreeMap::new(),
                },
                enabled_by_agent: McpEnabledByAgent {
                    codex: true,
                    claude: false,
                    project: false,
                },
                project_claude_target: ProjectClaudeTarget::WorkspaceMcpJson,
                status: SkillLifecycleStatus::Archived,
                archived_at: Some(String::from("2026-03-02T12:34:56.000Z")),
            },
        );

        let central_path = home.join(".config").join("ai-agents").join("config.toml");
        let wrapped = format!(
            "{CENTRAL_BEGIN}\n{}\n{CENTRAL_END}\n",
            render_central_block(&catalog)
        );
        std::fs::create_dir_all(central_path.parent().expect("parent")).expect("mkdir parent");
        std::fs::write(&central_path, wrapped).expect("write central");

        let outcome = registry.sync(&[]).expect("sync outcome");
        let codex_path = home.join(".codex").join("config.toml");
        let codex_raw = std::fs::read_to_string(&codex_path).expect("codex config");
        assert!(codex_raw.contains("active-exa"));
        assert!(!codex_raw.contains("archived-exa"));

        let active = outcome
            .records
            .iter()
            .find(|record| record.server_key == "active-exa")
            .expect("active record");
        let archived = outcome
            .records
            .iter()
            .find(|record| record.server_key == "archived-exa")
            .expect("archived record");
        assert_eq!(active.status, SkillLifecycleStatus::Active);
        assert_eq!(archived.status, SkillLifecycleStatus::Archived);
        assert!(archived.targets.is_empty());
    }

    #[test]
    fn mutate_catalog_entry_allows_project_scope_without_workspace_when_unambiguous() {
        let (_temp, registry, home, _runtime) = registry_in_temp();
        let workspace = String::from("/tmp/workspace-a");
        let project_catalog_id = format!("project::{workspace}::exa");

        let mut catalog = BTreeMap::new();
        catalog.insert(
            project_catalog_id.clone(),
            CatalogEntry {
                catalog_id: project_catalog_id.clone(),
                server_key: String::from("exa"),
                scope: McpScope::Project,
                workspace: Some(workspace),
                definition: McpDefinition {
                    transport: McpTransport::Http,
                    command: None,
                    args: vec![],
                    url: Some(String::from("https://mcp.exa.ai/mcp")),
                    env: BTreeMap::new(),
                },
                enabled_by_agent: McpEnabledByAgent {
                    codex: true,
                    claude: true,
                    project: true,
                },
                project_claude_target: ProjectClaudeTarget::WorkspaceMcpJson,
                status: SkillLifecycleStatus::Active,
                archived_at: None,
            },
        );
        write_central_catalog(&home, &catalog);

        registry
            .mutate_catalog_entry(&[], CatalogMutationAction::Delete, "exa", "project", None)
            .expect("project scope mutation without workspace should resolve when unique");
        let next_catalog = registry
            .load_central_catalog(&mut Vec::new())
            .expect("read next catalog");
        assert!(!next_catalog.contains_key(&project_catalog_id));
    }

    #[test]
    fn mutate_catalog_entry_delete_global_keeps_project_entry_with_same_server_key() {
        let (_temp, registry, home, _runtime) = registry_in_temp();
        let workspace = String::from("/tmp/workspace-a");
        let project_catalog_id = format!("project::{workspace}::exa");
        let global_catalog_id = String::from("global::exa");

        let mut catalog = BTreeMap::new();
        catalog.insert(
            global_catalog_id.clone(),
            CatalogEntry {
                catalog_id: global_catalog_id.clone(),
                server_key: String::from("exa"),
                scope: McpScope::Global,
                workspace: None,
                definition: McpDefinition {
                    transport: McpTransport::Http,
                    command: None,
                    args: vec![],
                    url: Some(String::from("https://mcp.exa.ai/mcp")),
                    env: BTreeMap::new(),
                },
                enabled_by_agent: McpEnabledByAgent {
                    codex: true,
                    claude: true,
                    project: false,
                },
                project_claude_target: ProjectClaudeTarget::WorkspaceMcpJson,
                status: SkillLifecycleStatus::Active,
                archived_at: None,
            },
        );
        catalog.insert(
            project_catalog_id.clone(),
            CatalogEntry {
                catalog_id: project_catalog_id.clone(),
                server_key: String::from("exa"),
                scope: McpScope::Project,
                workspace: Some(workspace.clone()),
                definition: McpDefinition {
                    transport: McpTransport::Http,
                    command: None,
                    args: vec![],
                    url: Some(String::from("https://mcp.exa.ai/mcp")),
                    env: BTreeMap::new(),
                },
                enabled_by_agent: McpEnabledByAgent {
                    codex: true,
                    claude: true,
                    project: true,
                },
                project_claude_target: ProjectClaudeTarget::WorkspaceMcpJson,
                status: SkillLifecycleStatus::Active,
                archived_at: None,
            },
        );
        write_central_catalog(&home, &catalog);

        registry
            .mutate_catalog_entry(&[], CatalogMutationAction::Delete, "exa", "global", None)
            .expect("delete global exa");

        let next_catalog = registry
            .load_central_catalog(&mut Vec::new())
            .expect("read next catalog");
        assert!(!next_catalog.contains_key(&global_catalog_id));
        assert!(next_catalog.contains_key(&project_catalog_id));
    }

    #[test]
    fn mutate_catalog_entry_make_global_moves_project_catalog_id_to_global() {
        let (_temp, registry, home, _runtime) = registry_in_temp();
        let workspace = String::from("/tmp/workspace-a");
        let project_catalog_id = format!("project::{workspace}::exa");

        let mut catalog = BTreeMap::new();
        catalog.insert(
            project_catalog_id.clone(),
            CatalogEntry {
                catalog_id: project_catalog_id.clone(),
                server_key: String::from("exa"),
                scope: McpScope::Project,
                workspace: Some(workspace.clone()),
                definition: McpDefinition {
                    transport: McpTransport::Http,
                    command: None,
                    args: vec![],
                    url: Some(String::from("https://mcp.exa.ai/mcp")),
                    env: BTreeMap::new(),
                },
                enabled_by_agent: McpEnabledByAgent {
                    codex: false,
                    claude: true,
                    project: true,
                },
                project_claude_target: ProjectClaudeTarget::WorkspaceMcpJson,
                status: SkillLifecycleStatus::Active,
                archived_at: None,
            },
        );
        write_central_catalog(&home, &catalog);

        registry
            .mutate_catalog_entry(
                &[],
                CatalogMutationAction::MakeGlobal,
                "exa",
                "project",
                Some(workspace.as_str()),
            )
            .expect("make global");

        let next_catalog = registry
            .load_central_catalog(&mut Vec::new())
            .expect("read next catalog");
        assert!(!next_catalog.contains_key(&project_catalog_id));
        assert!(next_catalog.contains_key("global::exa"));
        let promoted = next_catalog.get("global::exa").expect("promoted entry");
        assert_eq!(promoted.scope, McpScope::Global);
        assert_eq!(promoted.workspace, None);
    }

    #[test]
    fn mutate_catalog_entry_make_global_forces_project_agent_toggle_off() {
        let (_temp, registry, home, _runtime) = registry_in_temp();
        let workspace = String::from("/tmp/workspace-a");
        let project_catalog_id = format!("project::{workspace}::exa");

        let mut catalog = BTreeMap::new();
        catalog.insert(
            project_catalog_id,
            CatalogEntry {
                catalog_id: format!("project::{workspace}::exa"),
                server_key: String::from("exa"),
                scope: McpScope::Project,
                workspace: Some(workspace.clone()),
                definition: McpDefinition {
                    transport: McpTransport::Http,
                    command: None,
                    args: vec![],
                    url: Some(String::from("https://mcp.exa.ai/mcp")),
                    env: BTreeMap::new(),
                },
                enabled_by_agent: McpEnabledByAgent {
                    codex: true,
                    claude: false,
                    project: true,
                },
                project_claude_target: ProjectClaudeTarget::WorkspaceMcpJson,
                status: SkillLifecycleStatus::Active,
                archived_at: None,
            },
        );
        write_central_catalog(&home, &catalog);

        registry
            .mutate_catalog_entry(
                &[],
                CatalogMutationAction::MakeGlobal,
                "exa",
                "project",
                Some(workspace.as_str()),
            )
            .expect("make global");

        let next_catalog = registry
            .load_central_catalog(&mut Vec::new())
            .expect("read next catalog");
        let promoted = next_catalog.get("global::exa").expect("promoted entry");
        assert!(!promoted.enabled_by_agent.project);
        assert!(promoted.enabled_by_agent.codex);
        assert!(!promoted.enabled_by_agent.claude);
    }

    fn make_catalog_entry(
        catalog_id: &str,
        server_key: &str,
        scope: McpScope,
        workspace: Option<&str>,
        status: SkillLifecycleStatus,
    ) -> CatalogEntry {
        CatalogEntry {
            catalog_id: String::from(catalog_id),
            server_key: String::from(server_key),
            scope,
            workspace: workspace.map(String::from),
            definition: McpDefinition {
                transport: McpTransport::Stdio,
                command: Some(String::from("npx")),
                args: vec![String::from("-y"), String::from(server_key)],
                url: None,
                env: BTreeMap::new(),
            },
            enabled_by_agent: McpEnabledByAgent {
                codex: true,
                claude: true,
                project: scope == McpScope::Project,
            },
            project_claude_target: ProjectClaudeTarget::WorkspaceMcpJson,
            status,
            archived_at: None,
        }
    }

    #[test]
    fn sync_auto_cleans_stale_unmanaged_codex_entry() {
        let (_temp, registry, home, _runtime) = registry_in_temp();

        // Catalog has ahrefs under a project scope
        let mut catalog = BTreeMap::new();
        catalog.insert(
            String::from("project::/workspace::ahrefs"),
            make_catalog_entry(
                "project::/workspace::ahrefs",
                "ahrefs",
                McpScope::Project,
                Some("/workspace"),
                SkillLifecycleStatus::Active,
            ),
        );
        write_central_catalog(&home, &catalog);

        // Write unmanaged ahrefs entry into global codex config
        let codex_path = home.join(".codex").join("config.toml");
        std::fs::create_dir_all(codex_path.parent().unwrap()).unwrap();
        std::fs::write(
            &codex_path,
            "[mcp_servers.ahrefs]\ncommand = \"npx\"\nargs = [\"-y\", \"ahrefs\"]\n",
        )
        .unwrap();

        let outcome = registry.sync(&[]).expect("sync");

        // The unmanaged codex entry should be auto-cleaned
        let codex_raw = std::fs::read_to_string(&codex_path).unwrap_or_default();
        // The unmanaged block should no longer contain ahrefs
        let unmanaged = strip_managed_blocks(&codex_raw, &CODEX_MARKER_PAIRS);
        assert!(
            !unmanaged.contains("ahrefs"),
            "stale ahrefs should be removed from unmanaged codex block"
        );

        // No "unmanaged in central catalog" warning
        let unmanaged_warnings: Vec<_> = outcome
            .warnings
            .iter()
            .filter(|w| w.contains("unmanaged in central catalog") && w.contains("ahrefs"))
            .collect();
        assert!(
            unmanaged_warnings.is_empty(),
            "should not warn about stale entry managed elsewhere: {unmanaged_warnings:?}"
        );
    }

    #[test]
    fn sync_auto_cleans_stale_unmanaged_claude_global_entry() {
        let (_temp, registry, home, _runtime) = registry_in_temp();

        // Catalog has ahrefs under a project scope
        let mut catalog = BTreeMap::new();
        catalog.insert(
            String::from("project::/workspace::ahrefs"),
            make_catalog_entry(
                "project::/workspace::ahrefs",
                "ahrefs",
                McpScope::Project,
                Some("/workspace"),
                SkillLifecycleStatus::Active,
            ),
        );
        write_central_catalog(&home, &catalog);

        // Write unmanaged ahrefs in ~/.claude.json root mcpServers
        let claude_json_path = home.join(".claude.json");
        std::fs::write(
            &claude_json_path,
            r#"{"mcpServers":{"ahrefs":{"command":"npx","args":["-y","ahrefs"]}}}"#,
        )
        .unwrap();

        let outcome = registry.sync(&[]).expect("sync");

        // The ahrefs entry should be removed from claude.json
        let claude_raw = std::fs::read_to_string(&claude_json_path).unwrap();
        assert!(
            !claude_raw.contains("\"ahrefs\""),
            "stale ahrefs should be removed from claude.json root mcpServers"
        );

        let unmanaged_warnings: Vec<_> = outcome
            .warnings
            .iter()
            .filter(|w| w.contains("unmanaged in central catalog") && w.contains("ahrefs"))
            .collect();
        assert!(
            unmanaged_warnings.is_empty(),
            "should not warn about stale entry managed elsewhere: {unmanaged_warnings:?}"
        );
    }

    #[test]
    fn sync_auto_cleans_stale_unmanaged_claude_project_entry() {
        let (_temp, registry, home, _runtime) = registry_in_temp();

        let workspace = "/tmp/my-project";
        // Catalog has clarity under global scope
        let mut catalog = BTreeMap::new();
        catalog.insert(
            String::from("global::clarity"),
            make_catalog_entry(
                "global::clarity",
                "clarity",
                McpScope::Global,
                None,
                SkillLifecycleStatus::Active,
            ),
        );
        write_central_catalog(&home, &catalog);

        // Write unmanaged clarity in ~/.claude.json project section
        let claude_json_path = home.join(".claude.json");
        let claude_json = serde_json::json!({
            "projects": {
                workspace: {
                    "mcpServers": {
                        "clarity": {
                            "command": "npx",
                            "args": ["-y", "clarity"]
                        }
                    }
                }
            }
        });
        std::fs::write(
            &claude_json_path,
            serde_json::to_string_pretty(&claude_json).unwrap(),
        )
        .unwrap();

        let outcome = registry.sync(&[PathBuf::from(workspace)]).expect("sync");

        // clarity should be removed from the project section
        let claude_raw = std::fs::read_to_string(&claude_json_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&claude_raw).unwrap();
        let project_servers = parsed
            .get("projects")
            .and_then(|p| p.get(workspace))
            .and_then(|p| p.get("mcpServers"))
            .and_then(|s| s.as_object());
        if let Some(servers) = project_servers {
            assert!(
                !servers.contains_key("clarity"),
                "stale clarity should be removed from project mcpServers"
            );
        }

        let unmanaged_warnings: Vec<_> = outcome
            .warnings
            .iter()
            .filter(|w| w.contains("unmanaged in central catalog") && w.contains("clarity"))
            .collect();
        assert!(
            unmanaged_warnings.is_empty(),
            "should not warn about stale entry managed elsewhere: {unmanaged_warnings:?}"
        );
    }

    #[test]
    fn sync_keeps_warning_for_truly_unmanaged_entry() {
        let (_temp, registry, home, _runtime) = registry_in_temp();

        // Catalog has only "global::existing"
        let mut catalog = BTreeMap::new();
        catalog.insert(
            String::from("global::existing"),
            make_catalog_entry(
                "global::existing",
                "existing",
                McpScope::Global,
                None,
                SkillLifecycleStatus::Active,
            ),
        );
        write_central_catalog(&home, &catalog);

        // Write unmanaged "unknown-server" in codex (server_key not in catalog at all)
        let codex_path = home.join(".codex").join("config.toml");
        std::fs::create_dir_all(codex_path.parent().unwrap()).unwrap();
        std::fs::write(
            &codex_path,
            "[mcp_servers.unknown-server]\ncommand = \"npx\"\nargs = [\"-y\", \"unknown\"]\n",
        )
        .unwrap();

        let outcome = registry.sync(&[]).expect("sync");

        // Should still produce the "unmanaged" warning
        let unmanaged_warnings: Vec<_> = outcome
            .warnings
            .iter()
            .filter(|w| w.contains("unmanaged in central catalog") && w.contains("unknown-server"))
            .collect();
        assert!(
            !unmanaged_warnings.is_empty(),
            "should warn about truly unmanaged entry"
        );
    }

    #[test]
    fn is_secret_like_key_matches_known_patterns() {
        // Original patterns
        assert!(is_secret_like_key("AUTH_TOKEN"));
        assert!(is_secret_like_key("client_secret"));
        assert!(is_secret_like_key("DB_PASSWORD"));
        assert!(is_secret_like_key("MY_API_KEY"));
        assert!(is_secret_like_key("SOME_APIKEY"));

        // Newly added patterns
        assert!(is_secret_like_key("LICENSE"));
        assert!(is_secret_like_key("DAISYUI_LICENSE"));
        assert!(is_secret_like_key("license_key"));
        assert!(is_secret_like_key("EMAIL"));
        assert!(is_secret_like_key("DAISYUI_EMAIL"));
        assert!(is_secret_like_key("user_email"));
    }

    #[test]
    fn is_secret_like_key_rejects_non_secret_keys() {
        assert!(!is_secret_like_key("PATH"));
        assert!(!is_secret_like_key("HOME"));
        assert!(!is_secret_like_key("NODE_ENV"));
        assert!(!is_secret_like_key("DEBUG"));
        assert!(!is_secret_like_key("PORT"));
    }

    #[test]
    fn apply_codex_catalog_path_no_duplicate_when_unmanaged_and_managed_both_exist() {
        let (_temp, registry, home, _runtime) = registry_in_temp();
        let codex_path = home.join(".codex").join("config.toml");
        std::fs::create_dir_all(codex_path.parent().expect("parent")).expect("mkdir parent");

        // File already has manual exa/sentry AND a managed block with exa/sentry
        std::fs::write(
            &codex_path,
            "\
[mcp_servers.exa]\n\
command = \"npx\"\n\
args = [\"-y\", \"mcp-remote@latest\", \"https://mcp.exa.ai/mcp\"]\n\
\n\
[mcp_servers.sentry]\n\
command = \"npx\"\n\
args = [\"-y\", \"@sentry/mcp-server@latest\"]\n\
\n\
# agent-sync:mcp:codex:begin\n\
[mcp_servers.exa]\n\
command = \"npx\"\n\
args = [\"-y\", \"mcp-remote@latest\", \"https://mcp.exa.ai/mcp\"]\n\
enabled = true\n\
\n\
[mcp_servers.sentry]\n\
command = \"npx\"\n\
args = [\"-y\", \"@sentry/mcp-server@latest\"]\n\
enabled = true\n\
# agent-sync:mcp:codex:end\n",
        )
        .expect("write codex");

        let mut entries = BTreeMap::new();
        entries.insert(
            String::from("exa"),
            CodexCatalogEntry {
                definition: McpDefinition {
                    transport: McpTransport::Stdio,
                    command: Some(String::from("npx")),
                    args: vec![
                        String::from("-y"),
                        String::from("mcp-remote@latest"),
                        String::from("https://mcp.exa.ai/mcp"),
                    ],
                    url: None,
                    env: BTreeMap::new(),
                },
                enabled: true,
            },
        );
        entries.insert(
            String::from("sentry"),
            CodexCatalogEntry {
                definition: McpDefinition {
                    transport: McpTransport::Stdio,
                    command: Some(String::from("npx")),
                    args: vec![
                        String::from("-y"),
                        String::from("@sentry/mcp-server@latest"),
                    ],
                    url: None,
                    env: BTreeMap::new(),
                },
                enabled: false,
            },
        );

        let mut warnings = Vec::new();
        registry
            .apply_codex_catalog_path(&codex_path, &entries, true, &mut warnings)
            .expect("apply codex");

        let codex_raw = std::fs::read_to_string(&codex_path).expect("read codex");

        // exa is enabled → unmanaged entry wins, so it appears exactly once (unmanaged only)
        assert_eq!(
            count_occurrences(&codex_raw, "[mcp_servers.exa]"),
            1,
            "exa should appear exactly once; got:\n{codex_raw}"
        );
        // sentry is disabled → unmanaged is cleaned, managed block has disabled copy
        assert_eq!(
            count_occurrences(&codex_raw, "[mcp_servers.sentry]"),
            1,
            "sentry should appear exactly once; got:\n{codex_raw}"
        );
        // Output must be valid TOML
        assert!(
            toml::from_str::<toml::Table>(&codex_raw).is_ok(),
            "output must be valid TOML; got:\n{codex_raw}"
        );
    }

    #[test]
    fn apply_codex_catalog_path_no_duplicate_when_skills_block_between_mcp_entries() {
        let (_temp, registry, home, _runtime) = registry_in_temp();
        let codex_path = home.join(".codex").join("config.toml");
        std::fs::create_dir_all(codex_path.parent().expect("parent")).expect("mkdir parent");

        // File has manual exa, a skills block, and a managed MCP block with exa
        std::fs::write(
            &codex_path,
            "\
[mcp_servers.exa]\n\
command = \"npx\"\n\
args = [\"-y\", \"mcp-remote@latest\", \"https://mcp.exa.ai/mcp\"]\n\
\n\
# agent-sync:begin\n\
[[skills.config]]\n\
enabled = true\n\
path = \"/Users/test/.agents/skills/alpha\"\n\
\n\
[[skills.config]]\n\
enabled = true\n\
path = \"/Users/test/.agents/skills/beta\"\n\
# agent-sync:end\n\
\n\
# agent-sync:mcp:codex:begin\n\
[mcp_servers.exa]\n\
command = \"npx\"\n\
args = [\"-y\", \"mcp-remote@latest\", \"https://mcp.exa.ai/mcp\"]\n\
enabled = true\n\
# agent-sync:mcp:codex:end\n",
        )
        .expect("write codex");

        let mut entries = BTreeMap::new();
        entries.insert(
            String::from("exa"),
            CodexCatalogEntry {
                definition: McpDefinition {
                    transport: McpTransport::Stdio,
                    command: Some(String::from("npx")),
                    args: vec![
                        String::from("-y"),
                        String::from("mcp-remote@latest"),
                        String::from("https://mcp.exa.ai/mcp"),
                    ],
                    url: None,
                    env: BTreeMap::new(),
                },
                enabled: true,
            },
        );

        let mut warnings = Vec::new();
        registry
            .apply_codex_catalog_path(&codex_path, &entries, true, &mut warnings)
            .expect("apply codex");

        let codex_raw = std::fs::read_to_string(&codex_path).expect("read codex");

        assert_eq!(
            count_occurrences(&codex_raw, "[mcp_servers.exa]"),
            1,
            "exa should appear exactly once; got:\n{codex_raw}"
        );
        // Skills block must be preserved
        assert!(
            codex_raw.contains("[[skills.config]]"),
            "skills block should be preserved; got:\n{codex_raw}"
        );
        assert!(
            codex_raw.contains("/Users/test/.agents/skills/alpha"),
            "skills entries should be preserved"
        );
        // Output must be valid TOML
        assert!(
            toml::from_str::<toml::Table>(&codex_raw).is_ok(),
            "output must be valid TOML; got:\n{codex_raw}"
        );
    }
}
