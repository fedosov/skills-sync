import { describe, expect, it, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  getState,
  getSkillDetails,
  mutateSkill,
  openSkillPath,
  renameSkill,
  runSync,
} from "./tauriApi";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("tauriApi command payloads", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue(undefined);
  });

  it("sends camelCase payload for get_skill_details", async () => {
    await getSkillDetails("alpha");
    expect(invoke).toHaveBeenCalledWith("get_skill_details", {
      skillKey: "alpha",
    });
  });

  it("sends camelCase payload for rename_skill", async () => {
    await renameSkill("alpha", "New Title");
    expect(invoke).toHaveBeenCalledWith("rename_skill", {
      skillKey: "alpha",
      newTitle: "New Title",
    });
  });

  it("sends camelCase payload for mutation commands", async () => {
    await mutateSkill("archive_skill", "alpha");
    expect(invoke).toHaveBeenCalledWith("archive_skill", {
      skillKey: "alpha",
      confirmed: true,
    });
  });

  it("sends target payload for open_skill_path", async () => {
    await openSkillPath("alpha", "file");
    expect(invoke).toHaveBeenCalledWith("open_skill_path", {
      skillKey: "alpha",
      target: "file",
    });
  });

  it("runs sync with manual trigger", async () => {
    await runSync();
    expect(invoke).toHaveBeenCalledWith("run_sync", { trigger: "manual" });
  });

  it("loads state without args", async () => {
    await getState();
    expect(invoke).toHaveBeenCalledWith("get_state");
  });
});
