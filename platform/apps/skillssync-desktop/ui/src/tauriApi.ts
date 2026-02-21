import { invoke } from "@tauri-apps/api/core";
import type {
  AuditEvent,
  AuditQuery,
  DashboardSnapshot,
  DotagentsScope,
  MutationCommand,
  McpServerRecord,
  PlatformContext,
  RuntimeControls,
  SubagentDetails,
  SubagentRecord,
  SkillDetails,
  SkillRecord,
  SyncState,
} from "./types";

export async function getState(): Promise<SyncState> {
  return invoke<SyncState>("get_state");
}

export async function getStarredSkillIds(): Promise<string[]> {
  return invoke<string[]>("get_starred_skill_ids");
}

export async function loadDashboardSnapshot(): Promise<DashboardSnapshot> {
  const [state, starredSkillIds, subagents] = await Promise.all([
    getState(),
    getStarredSkillIds(),
    listSubagents("all"),
  ]);
  return {
    state,
    starredSkillIds,
    subagents,
  };
}

export async function setSkillStarred(
  skillId: string,
  starred: boolean,
): Promise<string[]> {
  return invoke<string[]>("set_skill_starred", {
    skillId,
    starred,
  });
}

export async function runSync(): Promise<SyncState> {
  return invoke<SyncState>("run_sync", { trigger: "manual" });
}

export async function runDotagentsSync(
  scope: DotagentsScope = "all",
): Promise<void> {
  return invoke<void>("run_dotagents_sync", { scope });
}

export async function getRuntimeControls(): Promise<RuntimeControls> {
  return invoke<RuntimeControls>("get_runtime_controls");
}

export async function setAllowFilesystemChanges(
  allow: boolean,
): Promise<RuntimeControls> {
  return invoke<RuntimeControls>("set_allow_filesystem_changes", { allow });
}

export async function listAuditEvents(
  query?: AuditQuery,
): Promise<AuditEvent[]> {
  const payload: {
    limit?: number;
    status?: string;
    action?: string;
  } = {};
  if (query?.limit) {
    payload.limit = query.limit;
  }
  if (query?.status) {
    payload.status = query.status;
  }
  if (query?.action?.trim()) {
    payload.action = query.action.trim();
  }
  return invoke<AuditEvent[]>("list_audit_events", payload);
}

export async function getSkillDetails(skillKey: string): Promise<SkillDetails> {
  return invoke<SkillDetails>("get_skill_details", { skillKey });
}

export async function getSubagentDetails(
  subagentId: string,
): Promise<SubagentDetails> {
  return invoke<SubagentDetails>("get_subagent_details", { subagentId });
}

export async function listSubagents(scope?: string): Promise<SubagentRecord[]> {
  return invoke<SubagentRecord[]>("list_subagents", { scope });
}

export async function listDotagentsSkills(
  scope: DotagentsScope = "all",
): Promise<SkillRecord[]> {
  return invoke<SkillRecord[]>("list_dotagents_skills", { scope });
}

export async function listDotagentsMcp(
  scope: DotagentsScope = "all",
): Promise<McpServerRecord[]> {
  return invoke<McpServerRecord[]>("list_dotagents_mcp", { scope });
}

export async function dotagentsSkillsInstall(
  scope: DotagentsScope = "all",
): Promise<void> {
  return invoke<void>("dotagents_skills_install", { scope });
}

export async function dotagentsSkillsAdd(
  packageName: string,
  scope: DotagentsScope = "all",
): Promise<void> {
  return invoke<void>("dotagents_skills_add", {
    package: packageName,
    scope,
  });
}

export async function dotagentsSkillsRemove(
  packageName: string,
  scope: DotagentsScope = "all",
): Promise<void> {
  return invoke<void>("dotagents_skills_remove", {
    package: packageName,
    scope,
  });
}

export async function dotagentsSkillsUpdate(
  packageName?: string,
  scope: DotagentsScope = "all",
): Promise<void> {
  return invoke<void>("dotagents_skills_update", {
    package: packageName,
    scope,
  });
}

export async function dotagentsMcpAdd(
  args: string[],
  scope: DotagentsScope = "all",
): Promise<void> {
  return invoke<void>("dotagents_mcp_add", { args, scope });
}

export async function dotagentsMcpRemove(
  args: string[],
  scope: DotagentsScope = "all",
): Promise<void> {
  return invoke<void>("dotagents_mcp_remove", { args, scope });
}

export async function migrateDotagents(
  scope: DotagentsScope = "all",
): Promise<void> {
  return invoke<void>("migrate_dotagents", { scope });
}

export async function getMcpServers(): Promise<McpServerRecord[]> {
  return invoke<McpServerRecord[]>("get_mcp_servers");
}

export async function setMcpServerEnabled(
  serverKey: string,
  agent: "codex" | "claude" | "project",
  enabled: boolean,
  scope?: "global" | "project",
  workspace?: string | null,
): Promise<SyncState> {
  const payload: {
    serverKey: string;
    agent: "codex" | "claude" | "project";
    enabled: boolean;
    scope?: "global" | "project";
    workspace?: string;
  } = {
    serverKey,
    agent,
    enabled,
  };
  if (scope) {
    payload.scope = scope;
  }
  if (workspace) {
    payload.workspace = workspace;
  }

  return invoke<SyncState>("set_mcp_server_enabled", {
    ...payload,
  });
}

export async function mutateSkill(
  command: MutationCommand,
  skillKey: string,
): Promise<SyncState> {
  return invoke<SyncState>(command, {
    skillKey,
    confirmed: true,
  });
}

export async function renameSkill(
  skillKey: string,
  newTitle: string,
): Promise<SyncState> {
  return invoke<SyncState>("rename_skill", {
    skillKey,
    newTitle,
  });
}

export async function openSkillPath(
  skillKey: string,
  target: "folder" | "file",
): Promise<void> {
  return invoke<void>("open_skill_path", {
    skillKey,
    target,
  });
}

export async function openSubagentPath(
  subagentId: string,
  target: "folder" | "file",
): Promise<void> {
  return invoke<void>("open_subagent_path", {
    subagentId,
    target,
  });
}

export async function getPlatformContext(): Promise<PlatformContext> {
  return invoke<PlatformContext>("get_platform_context");
}
