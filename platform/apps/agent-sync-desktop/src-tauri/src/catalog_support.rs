use agent_sync_core::{
    CatalogMutationAction, CatalogMutationTarget, SkillLifecycleStatus, SkillLocator, SkillRecord,
    SubagentRecord, SyncEngine, SyncState,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct RenameSkillResponse {
    pub(crate) state: SyncState,
    pub(crate) renamed_skill_key: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CatalogMutationActionPayload {
    Archive,
    Restore,
    Delete,
    MakeGlobal,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind")]
pub(crate) enum CatalogMutationTargetPayload {
    #[serde(rename = "skill")]
    Skill {
        #[serde(rename = "skillKey")]
        skill_key: String,
    },
    #[serde(rename = "subagent")]
    Subagent {
        #[serde(rename = "subagentId")]
        subagent_id: String,
    },
    #[serde(rename = "mcp")]
    Mcp {
        #[serde(rename = "serverKey")]
        server_key: String,
        scope: String,
        workspace: Option<String>,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CatalogMutationRequestPayload {
    pub(crate) action: CatalogMutationActionPayload,
    pub(crate) target: CatalogMutationTargetPayload,
    pub(crate) confirmed: bool,
}

impl From<CatalogMutationActionPayload> for CatalogMutationAction {
    fn from(value: CatalogMutationActionPayload) -> Self {
        match value {
            CatalogMutationActionPayload::Archive => Self::Archive,
            CatalogMutationActionPayload::Restore => Self::Restore,
            CatalogMutationActionPayload::Delete => Self::Delete,
            CatalogMutationActionPayload::MakeGlobal => Self::MakeGlobal,
        }
    }
}

pub(crate) fn validate_catalog_mutation_target(
    target: &CatalogMutationTargetPayload,
) -> Result<(), String> {
    match target {
        CatalogMutationTargetPayload::Skill { skill_key } => {
            if skill_key.trim().is_empty() {
                return Err(String::from("skillKey must be non-empty"));
            }
            Ok(())
        }
        CatalogMutationTargetPayload::Subagent { subagent_id } => {
            if subagent_id.trim().is_empty() {
                return Err(String::from("subagentId must be non-empty"));
            }
            Ok(())
        }
        CatalogMutationTargetPayload::Mcp {
            server_key,
            scope,
            workspace,
        } => {
            if server_key.trim().is_empty() {
                return Err(String::from("serverKey must be non-empty"));
            }
            let scope_value = scope.trim().to_ascii_lowercase();
            if scope_value != "global" && scope_value != "project" {
                return Err(format!("unsupported mcp scope: {scope} (global|project)"));
            }
            let workspace_present = workspace
                .as_ref()
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false);
            if scope_value == "global" && workspace_present {
                return Err(String::from(
                    "workspace must be omitted for mcp scope=global",
                ));
            }
            if scope_value == "project" && !workspace_present {
                return Err(String::from(
                    "workspace must be provided for mcp scope=project",
                ));
            }
            Ok(())
        }
    }
}

pub(crate) fn to_catalog_mutation_target(
    value: CatalogMutationTargetPayload,
) -> Result<CatalogMutationTarget, String> {
    validate_catalog_mutation_target(&value)?;
    match value {
        CatalogMutationTargetPayload::Skill { skill_key } => Ok(CatalogMutationTarget::Skill {
            skill_key: skill_key.trim().to_string(),
        }),
        CatalogMutationTargetPayload::Subagent { subagent_id } => {
            Ok(CatalogMutationTarget::Subagent {
                subagent_id: subagent_id.trim().to_string(),
            })
        }
        CatalogMutationTargetPayload::Mcp {
            server_key,
            scope,
            workspace,
        } => Ok(CatalogMutationTarget::Mcp {
            server_key: server_key.trim().to_string(),
            scope: scope.trim().to_ascii_lowercase(),
            workspace: normalize_optional_string(workspace),
        }),
    }
}

pub(crate) fn find_skill(
    engine: &SyncEngine,
    skill_key: &str,
    status: Option<SkillLifecycleStatus>,
) -> Result<SkillRecord, String> {
    engine
        .find_skill(&SkillLocator {
            skill_key: skill_key.to_owned(),
            status,
        })
        .ok_or_else(|| format!("skill not found: {skill_key}"))
}

pub(crate) fn find_subagent(
    engine: &SyncEngine,
    subagent_id: &str,
) -> Result<SubagentRecord, String> {
    engine
        .find_subagent_by_id(subagent_id)
        .ok_or_else(|| format!("subagent not found: {subagent_id}"))
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}
