import { invoke } from "@tauri-apps/api/core";
import type { MutationCommand, SkillDetails, SyncState } from "./types";

export async function getState(): Promise<SyncState> {
  return invoke<SyncState>("get_state");
}

export async function runSync(): Promise<SyncState> {
  return invoke<SyncState>("run_sync", { trigger: "manual" });
}

export async function getSkillDetails(skillKey: string): Promise<SkillDetails> {
  return invoke<SkillDetails>("get_skill_details", { skillKey });
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
