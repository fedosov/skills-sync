use serde::{Deserialize, Serialize};

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
    pub skills: Vec<SkillRecord>,
    #[serde(rename = "top_skills")]
    pub top_skills: Vec<String>,
}

impl SyncState {
    pub fn empty() -> Self {
        Self {
            version: 1,
            generated_at: String::new(),
            sync: SyncMetadata::empty(),
            summary: SyncSummary::empty(),
            skills: Vec::new(),
            top_skills: Vec::new(),
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
}

impl SyncMetadata {
    pub fn empty() -> Self {
        Self {
            status: SyncHealthStatus::Unknown,
            last_started_at: None,
            last_finished_at: None,
            duration_ms: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncSummary {
    #[serde(rename = "global_count")]
    pub global_count: usize,
    #[serde(rename = "project_count")]
    pub project_count: usize,
    #[serde(rename = "conflict_count")]
    pub conflict_count: usize,
}

impl SyncSummary {
    pub fn empty() -> Self {
        Self {
            global_count: 0,
            project_count: 0,
            conflict_count: 0,
        }
    }
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
    pub scope: String,
    pub workspace: Option<String>,
    #[serde(rename = "skill_key")]
    pub skill_key: String,
}
