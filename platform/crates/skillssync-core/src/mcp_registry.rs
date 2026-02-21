use crate::error::SyncEngineError;
use crate::models::{McpEnabledByAgent, McpServerRecord, McpTransport};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

const CENTRAL_BEGIN: &str = "# skills-sync:mcp:begin";
const CENTRAL_END: &str = "# skills-sync:mcp:end";
const CODEX_BEGIN: &str = "# skills-sync:mcp:codex:begin";
const CODEX_END: &str = "# skills-sync:mcp:codex:end";

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

#[derive(Debug, Clone)]
struct JsonWritePlan {
    path: PathBuf,
    location: JsonTargetLocation,
    create_when_missing: bool,
    entries: Vec<JsonCatalogEntry>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct McpManifest {
    version: u32,
    #[serde(rename = "generated_at")]
    generated_at: String,
    #[serde(default)]
    targets: BTreeMap<String, Vec<String>>,
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
            for item in &discovered {
                if !catalog.contains_key(&item.entry.catalog_id) {
                    warnings.push(format!(
                        "MCP server '{}' ({}) exists in {} but is unmanaged in central catalog",
                        item.entry.server_key,
                        item.entry.catalog_id,
                        item.file_path.display()
                    ));
                }
            }
        }
        self.reconcile_claude_enabled(&mut catalog, &discovered, &previous_manifest, &mut warnings);

        self.write_central_catalog(&catalog)?;

        let mut new_manifest = McpManifest {
            version: 3,
            generated_at: iso8601_now(),
            targets: BTreeMap::new(),
        };

        let global_codex_path = self.codex_config_path();
        let global_codex_defs = definitions_for_entries(
            catalog
                .values()
                .filter(|item| item.scope == McpScope::Global && item.enabled_by_agent.codex)
                .cloned()
                .collect::<Vec<_>>()
                .as_slice(),
        );
        let global_codex_keys = self.apply_codex_catalog_path(
            &global_codex_path,
            &global_codex_defs,
            true,
            &mut warnings,
        )?;
        new_manifest
            .targets
            .insert(global_codex_path.display().to_string(), global_codex_keys);

        let global_claude_target = self.effective_global_claude_target_path();
        let claude_user_path = self.claude_user_config_path();

        let mut json_plans: Vec<JsonWritePlan> = Vec::new();
        for item in catalog.values() {
            if item.scope == McpScope::Global && item.enabled_by_agent.claude {
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

            if item.scope != McpScope::Project
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
            let project_codex_defs = definitions_for_entries(
                catalog
                    .values()
                    .filter(|item| {
                        item.scope == McpScope::Project
                            && item.workspace.as_deref() == Some(workspace_key.as_str())
                            && item.enabled_by_agent.project
                            && item.enabled_by_agent.codex
                    })
                    .cloned()
                    .collect::<Vec<_>>()
                    .as_slice(),
            );
            if project_codex_path.exists() {
                let keys = self.apply_codex_catalog_path(
                    &project_codex_path,
                    &project_codex_defs,
                    false,
                    &mut warnings,
                )?;
                new_manifest
                    .targets
                    .insert(project_codex_path.display().to_string(), keys);
            } else if !project_codex_defs.is_empty() || !previous_project_codex_keys.is_empty() {
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
                    targets,
                    warnings: {
                        record_warnings.sort();
                        record_warnings.dedup();
                        record_warnings
                    },
                }
            })
            .collect::<Vec<_>>();
        records.sort_by(|lhs, rhs| {
            lhs.server_key
                .cmp(&rhs.server_key)
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

        let Some(body) = extract_managed_block(&raw, CENTRAL_BEGIN, CENTRAL_END) else {
            return Ok(BTreeMap::new());
        };
        if body.trim().is_empty() {
            return Ok(BTreeMap::new());
        }

        let parsed = body.parse::<toml::Value>().map_err(|error| {
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
        let block = render_central_block(catalog);
        let updated = upsert_managed_block(&existing, CENTRAL_BEGIN, CENTRAL_END, &block);
        if updated == existing {
            return Ok(());
        }
        fs::write(&path, updated).map_err(|error| SyncEngineError::io(&path, error))
    }

    fn apply_codex_catalog_path(
        &self,
        path: &Path,
        definitions: &BTreeMap<String, McpDefinition>,
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

        let unmanaged = strip_managed_block(&existing, CODEX_BEGIN, CODEX_END);
        let unmanaged_keys = read_toml_server_keys_from_str(&unmanaged).unwrap_or_default();
        let mut filtered = BTreeMap::new();
        for (key, definition) in definitions {
            if unmanaged_keys.contains(key) {
                warnings.push(format!(
                    "Skipped managed Codex MCP '{}' because unmanaged entry already exists in {}",
                    key,
                    path.display()
                ));
                continue;
            }
            filtered.insert(key.clone(), definition.clone());
        }

        if let Some(parent) = path.parent() {
            if create_when_missing || path.exists() {
                fs::create_dir_all(parent).map_err(|error| SyncEngineError::io(parent, error))?;
            }
        }

        let block = render_codex_block(&filtered);
        let updated = upsert_managed_block(&existing, CODEX_BEGIN, CODEX_END, &block);
        if updated != existing {
            fs::write(path, updated).map_err(|error| SyncEngineError::io(path, error))?;
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

        let mut rendered = serde_json::to_vec_pretty(&root)?;
        rendered.push(b'\n');
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
        let mut data = serde_json::to_vec_pretty(manifest)?;
        data.push(b'\n');
        fs::write(&path, data).map_err(|error| SyncEngineError::io(path, error))
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

fn definitions_for_entries(entries: &[CatalogEntry]) -> BTreeMap<String, McpDefinition> {
    let mut definitions = BTreeMap::new();
    for item in entries {
        definitions.insert(item.server_key.clone(), item.definition.clone());
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
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }

    let parsed = raw
        .parse::<toml::Value>()
        .map_err(|error| error.to_string())?;
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

fn read_toml_server_keys_from_str(raw: &str) -> Result<HashSet<String>, String> {
    let servers = read_toml_servers_from_str(raw)?;
    Ok(servers
        .into_iter()
        .map(|(key, _, _)| key)
        .collect::<HashSet<_>>())
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

    let mut lines = Vec::new();
    for (catalog_id, entry) in catalog {
        lines.push(format!("[mcp_catalog.\"{}\"]", toml_escape(catalog_id)));
        lines.push(format!(
            "server_key = \"{}\"",
            toml_escape(&entry.server_key)
        ));
        lines.push(format!("scope = \"{}\"", entry.scope.as_str()));
        if let Some(workspace) = &entry.workspace {
            lines.push(format!("workspace = \"{}\"", toml_escape(workspace)));
            lines.push(format!(
                "project_claude_target = \"{}\"",
                entry.project_claude_target.as_str()
            ));
        }
        lines.push(format!(
            "transport = \"{}\"",
            match entry.definition.transport {
                McpTransport::Stdio => "stdio",
                McpTransport::Http => "http",
            }
        ));
        if let Some(command) = &entry.definition.command {
            lines.push(format!("command = \"{}\"", toml_escape(command)));
        }
        if !entry.definition.args.is_empty() {
            lines.push(format!(
                "args = [{}]",
                entry
                    .definition
                    .args
                    .iter()
                    .map(|value| format!("\"{}\"", toml_escape(value)))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if let Some(url) = &entry.definition.url {
            lines.push(format!("url = \"{}\"", toml_escape(url)));
        }
        if !entry.definition.env.is_empty() {
            lines.push(format!("[mcp_catalog.\"{}\".env]", toml_escape(catalog_id)));
            for (env_key, env_value) in &entry.definition.env {
                lines.push(format!("{} = \"{}\"", env_key, toml_escape(env_value)));
            }
        }
        lines.push(format!(
            "[mcp_catalog.\"{}\".enabled_by_agent]",
            toml_escape(catalog_id)
        ));
        lines.push(format!("codex = {}", entry.enabled_by_agent.codex));
        lines.push(format!("claude = {}", entry.enabled_by_agent.claude));
        lines.push(format!("project = {}", entry.enabled_by_agent.project));
        lines.push(String::new());
    }
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }

    lines.join("\n")
}

fn render_codex_block(definitions: &BTreeMap<String, McpDefinition>) -> String {
    if definitions.is_empty() {
        return "# No managed MCP entries".to_string();
    }

    let mut lines = Vec::new();
    for (key, definition) in definitions {
        lines.push(format!("[mcp_servers.\"{}\"]", toml_escape(key)));
        if let Some(command) = &definition.command {
            lines.push(format!("command = \"{}\"", toml_escape(command)));
        }
        if !definition.args.is_empty() {
            lines.push(format!(
                "args = [{}]",
                definition
                    .args
                    .iter()
                    .map(|value| format!("\"{}\"", toml_escape(value)))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if let Some(url) = &definition.url {
            lines.push(format!("url = \"{}\"", toml_escape(url)));
        }
        if !definition.env.is_empty() {
            lines.push(format!("[mcp_servers.\"{}\".env]", toml_escape(key)));
            for (env_key, env_value) in &definition.env {
                lines.push(format!("{} = \"{}\"", env_key, toml_escape(env_value)));
            }
        }
        lines.push(String::from("enabled = true"));
        lines.push(String::new());
    }
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }

    lines.join("\n")
}

fn detect_inline_secret_warnings(server_key: &str, definition: &McpDefinition) -> Vec<String> {
    let mut warnings = Vec::new();
    for (key, value) in &definition.env {
        let key_lower = key.to_ascii_lowercase();
        if !(key_lower.contains("token")
            || key_lower.contains("secret")
            || key_lower.contains("password")
            || key_lower.contains("api_key"))
        {
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
        let lower = arg.to_ascii_lowercase();
        if (lower.contains("token=") || lower.contains("secret=") || lower.contains("api_key="))
            && !arg.contains("${")
        {
            warnings.push(format!(
                "MCP server '{}' has inline secret-like argument '{}'",
                server_key, arg
            ));
        }
    }
    warnings
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

fn strip_managed_block(current: &str, begin_marker: &str, end_marker: &str) -> String {
    let normalized = current.replace("\r\n", "\n");
    let Some(begin_index) = normalized.find(begin_marker) else {
        return normalized;
    };
    let Some(end_index) = normalized[begin_index..].find(end_marker) else {
        return normalized;
    };
    let end_absolute = begin_index + end_index + end_marker.len();
    let prefix = normalized[..begin_index].trim_matches('\n');
    let suffix = normalized[end_absolute..].trim_matches('\n');
    match (prefix.is_empty(), suffix.is_empty()) {
        (true, true) => String::new(),
        (true, false) => format!("{suffix}\n"),
        (false, true) => format!("{prefix}\n"),
        (false, false) => format!("{prefix}\n\n{suffix}\n"),
    }
}

fn upsert_managed_block(current: &str, begin_marker: &str, end_marker: &str, body: &str) -> String {
    let block = format!("{begin_marker}\n{body}\n{end_marker}");
    if current.trim().is_empty() {
        return format!("{block}\n");
    }

    let normalized = current.replace("\r\n", "\n");
    if let Some(begin_index) = normalized.find(begin_marker) {
        if let Some(end_index) = normalized[begin_index..].find(end_marker) {
            let end_absolute = begin_index + end_index + end_marker.len();
            let prefix = normalized[..begin_index].trim_matches('\n');
            let suffix = normalized[end_absolute..].trim_matches('\n');
            return match (prefix.is_empty(), suffix.is_empty()) {
                (true, true) => format!("{block}\n"),
                (true, false) => format!("{block}\n\n{suffix}\n"),
                (false, true) => format!("{prefix}\n\n{block}\n"),
                (false, false) => format!("{prefix}\n\n{block}\n\n{suffix}\n"),
            };
        }
    }

    let trimmed = normalized.trim_matches('\n');
    format!("{trimmed}\n\n{block}\n")
}

fn toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn iso8601_now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::{read_json_servers, read_toml_servers_from_str, upsert_managed_block};

    #[test]
    fn upsert_managed_block_replaces_existing() {
        let current =
            "alpha = true\n\n# skills-sync:mcp:begin\nold = true\n# skills-sync:mcp:end\n";
        let next = upsert_managed_block(
            current,
            "# skills-sync:mcp:begin",
            "# skills-sync:mcp:end",
            "new = true",
        );
        assert!(next.contains("new = true"));
        assert!(!next.contains("old = true"));
        assert!(next.contains("alpha = true"));
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
}
