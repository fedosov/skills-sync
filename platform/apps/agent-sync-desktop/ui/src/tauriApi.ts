import { invoke } from "@tauri-apps/api/core";
import type {
  AppContext,
  DotagentsCommandRequest,
  DotagentsCommandResult,
  DotagentsMcpListItem,
  DotagentsRuntimeStatus,
  DotagentsScope,
  DotagentsSkillListItem,
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
