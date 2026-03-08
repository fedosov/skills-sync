pub mod agents_context;
pub mod audit_store;
pub mod codex_registry;
pub mod codex_subagent_registry;
pub mod config_validation;
pub mod dotagents_adapter;
pub mod dotagents_runtime;
pub mod engine;
pub mod error;
pub mod managed_block;
pub mod mcp_registry;
pub mod models;
pub mod paths;
pub mod settings;
pub mod state_store;
mod toml_scan;
pub mod watch;

pub use agents_context::{
    AgentContextEntry, AgentContextSegment, AgentContextSeverity, AgentsContextLimits,
    AgentsContextReport, AgentsContextTotals,
};
pub use audit_store::{SyncAuditStore, DEFAULT_AUDIT_LOG_LIMIT};
pub use engine::{
    DotagentsScope, RenameSkillResult, ScopeFilter, SkillLocator, SyncEngine, SyncEngineEnvironment,
};
pub use error::{load_json_or_default, render_json_pretty, write_json_pretty, SyncEngineError};
pub use mcp_registry::{McpAgent, UnmanagedClaudeMcpCandidate, UnmanagedClaudeMcpFixReport};
pub use models::{
    AuditEvent, AuditEventStatus, CatalogMutationAction, CatalogMutationRequest,
    CatalogMutationTarget, ConfigFormat, ConfigValidationResult, McpEnabledByAgent,
    McpServerRecord, McpTransport, SkillLifecycleStatus, SkillRecord, SubagentRecord, SyncConflict,
    SyncHealthStatus, SyncMetadata, SyncState, SyncSummary, SyncTrigger,
};
pub use paths::SyncPaths;
pub use settings::{SyncAppSettings, SyncPreferencesStore};
pub use state_store::SyncStateStore;
