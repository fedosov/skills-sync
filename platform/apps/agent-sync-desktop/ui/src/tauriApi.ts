import { invoke } from "@tauri-apps/api/core";
import type {
  AppContext,
  DotagentsCommandRequest,
  DotagentsCommandResult,
  DotagentsMcpListItem,
  DotagentsRuntimeStatus,
  DotagentsScope,
  DotagentsSkillListItem,
  SkillsCliCommandRequest,
  SkillsCliCommandResult,
  SkillsCliListItem,
  SkillsCliScope,
  SkillsWorkspaceContext,
} from "./types";

export async function getRuntimeStatus(): Promise<DotagentsRuntimeStatus> {
  return invoke<DotagentsRuntimeStatus>("get_runtime_status");
}

export async function getAppContext(): Promise<AppContext> {
  return invoke<AppContext>("get_app_context");
}

export async function setScope(scope: DotagentsScope): Promise<AppContext> {
  return invoke<AppContext>("set_scope", { scope });
}

export async function setProjectRoot(
  projectRoot: string | null,
): Promise<AppContext> {
  return invoke<AppContext>("set_project_root", { projectRoot });
}

export async function listSkills(): Promise<DotagentsSkillListItem[]> {
  return invoke<DotagentsSkillListItem[]>("list_skills");
}

export async function listMcpServers(): Promise<DotagentsMcpListItem[]> {
  return invoke<DotagentsMcpListItem[]>("list_mcp_servers");
}

export async function runDotagentsCommand(
  request: DotagentsCommandRequest,
): Promise<DotagentsCommandResult> {
  return invoke<DotagentsCommandResult>("run_dotagents_command", { request });
}

export async function openAgentsToml(): Promise<void> {
  return invoke<void>("open_agents_toml");
}

export async function openAgentsDir(): Promise<void> {
  return invoke<void>("open_agents_dir");
}

export async function openUserHome(): Promise<void> {
  return invoke<void>("open_user_home");
}

// ---------------------------------------------------------------------------
// Skills Workspace
// ---------------------------------------------------------------------------

export async function getSkillsWorkspaceContext(): Promise<SkillsWorkspaceContext> {
  return invoke<SkillsWorkspaceContext>("get_skills_workspace_context");
}

export async function setSkillsScope(
  scope: SkillsCliScope,
): Promise<SkillsWorkspaceContext> {
  return invoke<SkillsWorkspaceContext>("set_skills_scope", { scope });
}

export async function setSkillsActiveAgents(
  agents: string[],
): Promise<SkillsWorkspaceContext> {
  return invoke<SkillsWorkspaceContext>("set_skills_active_agents", { agents });
}

export async function setSkillsVersionOverride(
  versionOverride: string | null,
): Promise<SkillsWorkspaceContext> {
  return invoke<SkillsWorkspaceContext>("set_skills_version_override", {
    versionOverride,
  });
}

export async function listSkillsCli(): Promise<SkillsCliListItem[]> {
  return invoke<SkillsCliListItem[]>("list_skills_cli");
}

export async function runSkillsCliCommand(
  request: SkillsCliCommandRequest,
): Promise<SkillsCliCommandResult> {
  return invoke<SkillsCliCommandResult>("run_skills_cli_command", { request });
}
