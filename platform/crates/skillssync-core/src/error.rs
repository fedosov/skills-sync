use crate::models::SyncConflict;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SyncEngineError {
    #[error("Detected {0} skill conflict(s)")]
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
