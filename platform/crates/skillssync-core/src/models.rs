use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncHealthStatus {
    Ok,
    Failed,
    Syncing,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncState {
    pub version: u32,
    #[serde(rename = "generated_at")]
    pub generated_at: String,
    pub sync: SyncMetadata,
    pub summary: SyncSummary,
    #[serde(default, rename = "subagent_summary")]
    pub subagent_summary: SyncSummary,
    pub skills: Vec<SkillRecord>,
    #[serde(default)]
    pub subagents: Vec<SubagentRecord>,
    #[serde(default, rename = "mcp_servers")]
    pub mcp_servers: Vec<McpServerRecord>,
    #[serde(rename = "top_skills")]
    pub top_skills: Vec<String>,
    #[serde(default, rename = "top_subagents")]
    pub top_subagents: Vec<String>,
}

impl SyncState {
    pub fn empty() -> Self {
        Self {
            version: 2,
            generated_at: String::new(),
            sync: SyncMetadata::empty(),
            summary: SyncSummary::empty(),
            subagent_summary: SyncSummary::empty(),
            skills: Vec::new(),
            subagents: Vec::new(),
            mcp_servers: Vec::new(),
            top_skills: Vec::new(),
            top_subagents: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncMetadata {
    pub status: SyncHealthStatus,
    #[serde(rename = "last_started_at")]
    pub last_started_at: Option<String>,
    #[serde(rename = "last_finished_at")]
    pub last_finished_at: Option<String>,
    #[serde(rename = "duration_ms")]
    pub duration_ms: Option<u64>,
    pub error: Option<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

impl SyncMetadata {
    pub fn empty() -> Self {
        Self {
            status: SyncHealthStatus::Unknown,
            last_started_at: None,
            last_finished_at: None,
            duration_ms: None,
            error: None,
            warnings: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SyncSummary {
    #[serde(rename = "global_count")]
    pub global_count: usize,
    #[serde(rename = "project_count")]
    pub project_count: usize,
    #[serde(rename = "conflict_count")]
    pub conflict_count: usize,
    #[serde(default, rename = "mcp_count")]
    pub mcp_count: usize,
    #[serde(default, rename = "mcp_warning_count")]
    pub mcp_warning_count: usize,
}

impl SyncSummary {
    pub fn empty() -> Self {
        Self {
            global_count: 0,
            project_count: 0,
            conflict_count: 0,
            mcp_count: 0,
            mcp_warning_count: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpEnabledByAgent {
    #[serde(default = "default_true")]
    pub codex: bool,
    #[serde(default = "default_true")]
    pub claude: bool,
    #[serde(default = "default_true")]
    pub project: bool,
}

impl Default for McpEnabledByAgent {
    fn default() -> Self {
        Self {
            codex: true,
            claude: true,
            project: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpTransport {
    Stdio,
    Http,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServerRecord {
    #[serde(rename = "server_key")]
    pub server_key: String,
    #[serde(default = "default_mcp_scope")]
    pub scope: String,
    #[serde(default)]
    pub workspace: Option<String>,
    pub transport: McpTransport,
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    pub url: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(rename = "enabled_by_agent")]
    pub enabled_by_agent: McpEnabledByAgent,
    #[serde(default)]
    pub targets: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

fn default_true() -> bool {
    true
}

fn default_mcp_scope() -> String {
    String::from("global")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillLifecycleStatus {
    Active,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SkillRecord {
    pub id: String,
    pub name: String,
    pub scope: String,
    pub workspace: Option<String>,
    #[serde(rename = "canonical_source_path")]
    pub canonical_source_path: String,
    #[serde(rename = "target_paths")]
    pub target_paths: Vec<String>,
    pub exists: bool,
    #[serde(rename = "is_symlink_canonical")]
    pub is_symlink_canonical: bool,
    #[serde(rename = "package_type")]
    pub package_type: String,
    #[serde(rename = "skill_key")]
    pub skill_key: String,
    #[serde(rename = "symlink_target")]
    pub symlink_target: String,
    #[serde(default = "default_skill_status")]
    pub status: SkillLifecycleStatus,
    #[serde(rename = "archived_at")]
    pub archived_at: Option<String>,
    #[serde(rename = "archived_bundle_path")]
    pub archived_bundle_path: Option<String>,
    #[serde(rename = "archived_original_scope")]
    pub archived_original_scope: Option<String>,
    #[serde(rename = "archived_original_workspace")]
    pub archived_original_workspace: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubagentRecord {
    pub id: String,
    pub name: String,
    pub description: String,
    pub scope: String,
    pub workspace: Option<String>,
    #[serde(rename = "canonical_source_path")]
    pub canonical_source_path: String,
    #[serde(rename = "target_paths")]
    pub target_paths: Vec<String>,
    pub exists: bool,
    #[serde(rename = "is_symlink_canonical")]
    pub is_symlink_canonical: bool,
    #[serde(rename = "package_type")]
    pub package_type: String,
    #[serde(rename = "subagent_key")]
    pub subagent_key: String,
    #[serde(rename = "symlink_target")]
    pub symlink_target: String,
    pub model: Option<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default, rename = "codex_tools_ignored")]
    pub codex_tools_ignored: bool,
}

fn default_skill_status() -> SkillLifecycleStatus {
    SkillLifecycleStatus::Active
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncTrigger {
    Manual,
    Widget,
    Delete,
    Archive,
    Restore,
    MakeGlobal,
    Rename,
    AutoFilesystem,
}

impl SyncTrigger {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Widget => "widget",
            Self::Delete => "delete",
            Self::Archive => "archive",
            Self::Restore => "restore",
            Self::MakeGlobal => "make_global",
            Self::Rename => "rename",
            Self::AutoFilesystem => "auto-filesystem",
        }
    }
}

impl TryFrom<&str> for SyncTrigger {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.trim().to_ascii_lowercase().as_str() {
            "manual" => Ok(Self::Manual),
            "widget" => Ok(Self::Widget),
            "delete" => Ok(Self::Delete),
            "archive" => Ok(Self::Archive),
            "restore" => Ok(Self::Restore),
            "make_global" | "make-global" => Ok(Self::MakeGlobal),
            "rename" => Ok(Self::Rename),
            "auto-filesystem" | "auto_filesystem" => Ok(Self::AutoFilesystem),
            other => Err(format!("unsupported trigger: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncConflict {
    #[serde(default)]
    pub kind: SyncConflictKind,
    pub scope: String,
    pub workspace: Option<String>,
    #[serde(rename = "skill_key")]
    pub skill_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SyncConflictKind {
    #[default]
    Skill,
    Subagent,
}
