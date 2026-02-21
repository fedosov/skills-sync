import { invoke } from "@tauri-apps/api/core";
import type {
  MutationCommand,
  McpServerRecord,
  PlatformContext,
  SubagentDetails,
  SubagentRecord,
  SkillDetails,
  SyncState,
} from "./types";

export async function getState(): Promise<SyncState> {
  return invoke<SyncState>("get_state");
}

export async function getStarredSkillIds(): Promise<string[]> {
  return invoke<string[]>("get_starred_skill_ids");
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
