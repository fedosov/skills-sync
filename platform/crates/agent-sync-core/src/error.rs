use crate::models::SyncConflict;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SyncEngineError {
    #[error("Detected {0} conflict(s)")]
    Conflicts(usize, Vec<SyncConflict>),

    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("delete_canonical_source requires confirmed=true")]
    DeleteRequiresConfirmation,
    #[error("Deletion blocked for protected path")]
    DeletionBlockedProtectedPath,
    #[error("Deletion blocked: target outside allowed roots")]
    DeletionOutsideAllowedRoots,
    #[error("Deletion target does not exist")]
    DeletionTargetMissing,

    #[error("archive_canonical_source requires confirmed=true")]
    ArchiveRequiresConfirmation,
    #[error("archive_canonical_source is only allowed for active skills")]
    ArchiveOnlyForActiveSkill,
    #[error("Archive blocked for protected path")]
    ArchiveBlockedProtectedPath,
    #[error("Archive blocked: source outside allowed roots")]
    ArchiveOutsideAllowedRoots,
    #[error("Archive source does not exist")]
    ArchiveSourceMissing,
    #[error("Archive manifest write failed")]
    ArchiveManifestWriteFailed,

    #[error("restore_archived_skill_to_global requires confirmed=true")]
    RestoreRequiresConfirmation,
    #[error("restore_archived_skill_to_global is only allowed for archived skills")]
    RestoreOnlyForArchivedSkill,
    #[error("Archived bundle path is missing")]
    RestoreBundleMissing,
    #[error("Archived manifest is missing or invalid")]
    RestoreManifestMissing,
    #[error("Archived source payload is missing")]
    RestoreSourceMissing,
    #[error("Restore target already exists")]
    RestoreTargetExists,
    #[error("archive_subagent requires confirmed=true")]
    ArchiveSubagentRequiresConfirmation,
    #[error("archive_subagent is only allowed for active subagents")]
    ArchiveOnlyForActiveSubagent,
    #[error("Archive subagent blocked for protected path")]
    ArchiveSubagentBlockedProtectedPath,
    #[error("Archive subagent blocked: source outside allowed roots")]
    ArchiveSubagentOutsideAllowedRoots,
    #[error("Archive subagent source does not exist")]
    ArchiveSubagentSourceMissing,
    #[error("Archive subagent manifest write failed")]
    ArchiveSubagentManifestWriteFailed,

    #[error("restore_archived_subagent requires confirmed=true")]
    RestoreSubagentRequiresConfirmation,
    #[error("restore_archived_subagent is only allowed for archived subagents")]
    RestoreOnlyForArchivedSubagent,
    #[error("Archived subagent bundle path is missing")]
    RestoreSubagentBundleMissing,
    #[error("Archived subagent manifest is missing or invalid")]
    RestoreSubagentManifestMissing,
    #[error("Archived subagent source payload is missing")]
    RestoreSubagentSourceMissing,
    #[error("Restore subagent target already exists")]
    RestoreSubagentTargetExists,
    #[error("Restore subagent target is outside allowed roots")]
    RestoreSubagentOutsideAllowedRoots,
    #[error("Restore subagent blocked for protected path")]
    RestoreSubagentBlockedProtectedPath,

    #[error("delete_subagent requires confirmed=true")]
    DeleteSubagentRequiresConfirmation,
    #[error("Deletion subagent blocked for protected path")]
    DeletionSubagentBlockedProtectedPath,
    #[error("Deletion subagent blocked: target outside allowed roots")]
    DeletionSubagentOutsideAllowedRoots,
    #[error("Deletion subagent target does not exist")]
    DeletionSubagentTargetMissing,

    #[error("make_global requires confirmed=true")]
    MakeGlobalRequiresConfirmation,
    #[error("make_global is only allowed for project skills")]
    MakeGlobalOnlyForProject,
    #[error("Make global blocked for protected path")]
    MakeGlobalBlockedProtectedPath,
    #[error("Make global blocked: source outside project roots")]
    MakeGlobalOutsideAllowedRoots,
    #[error("Make global source does not exist")]
    MakeGlobalSourceMissing,
    #[error("Make global target already exists")]
    MakeGlobalTargetExists,

    #[error("rename requires a non-empty title that produces a valid key")]
    RenameRequiresNonEmptyTitle,
    #[error("rename source does not exist")]
    RenameRequiresExistingSource,
    #[error("Rename blocked for protected path")]
    RenameBlockedProtectedPath,
    #[error("Rename blocked: source outside allowed roots")]
    RenameOutsideAllowedRoots,
    #[error("Rename blocked: target already exists")]
    RenameConflictTargetExists,
    #[error("Rename is a no-op: generated key is unchanged")]
    RenameNoOp,

    #[error("Failed to update Codex skills registry: {0}")]
    CodexRegistryWriteFailed(String),

    #[error("Migration failed for {skill_key}: {reason}")]
    MigrationFailed { skill_key: String, reason: String },

    #[error("dotagents binary unavailable: {0}")]
    DotagentsUnavailable(String),

    #[error("dotagents checksum mismatch for {path}: expected {expected}, got {actual}")]
    DotagentsChecksumMismatch {
        path: PathBuf,
        expected: String,
        actual: String,
    },

    #[error("dotagents version mismatch: expected {expected}, got {actual}")]
    DotagentsVersionMismatch { expected: String, actual: String },

    #[error(
        "dotagents command failed: {command} (exit={exit_code:?}); stderr={stderr}; stdout={stdout}"
    )]
    DotagentsCommandFailed {
        command: String,
        exit_code: Option<i32>,
        stderr: String,
        stdout: String,
    },

    #[error(
        "dotagents init already has agents.toml for {scope} scope at {cwd}; run Verify dotagents or `agent-sync migrate-dotagents --scope {scope}`"
    )]
    DotagentsInitAlreadyExists { scope: String, cwd: PathBuf },

    #[error("strict contract missing: {0}")]
    StrictContractMissing(String),

    #[error("migration required before strict dotagents sync: {0}")]
    MigrationRequired(String),

    #[error("mutate_catalog_item requires confirmed=true")]
    CatalogMutationRequiresConfirmation,
    #[error("MCP catalog mutation supports only scope global|project, got '{scope}'")]
    McpMutationInvalidScope { scope: String },
    #[error("MCP catalog entry not found for '{server_key}' in scope '{scope}'")]
    McpCatalogEntryNotFound { server_key: String, scope: String },
    #[error(
        "ambiguous MCP catalog locator for '{server_key}' in scope '{scope}', provide workspace"
    )]
    McpCatalogEntryAmbiguous { server_key: String, scope: String },
    #[error("MCP archive is only allowed for active entries")]
    McpArchiveOnlyForActive,
    #[error("MCP restore is only allowed for archived entries")]
    McpRestoreOnlyForArchived,
    #[error("MCP make_global is only allowed for active entries")]
    McpMakeGlobalOnlyForActive,
    #[error("MCP make_global is only allowed for project entries")]
    McpMakeGlobalOnlyForProject,
    #[error("MCP make_global target already exists for '{server_key}'")]
    McpMakeGlobalTargetExists { server_key: String },

    #[error("Unsupported platform operation: {0}")]
    Unsupported(String),
}

impl SyncEngineError {
    pub fn conflicts(conflicts: Vec<SyncConflict>) -> Self {
        Self::Conflicts(conflicts.len(), conflicts)
    }

    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
