import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import * as tauriApi from "./tauriApi";
import type { SkillDetails, SkillRecord, SyncState } from "./types";

vi.mock("./tauriApi", () => ({
  getState: vi.fn(),
  runSync: vi.fn(),
  getSkillDetails: vi.fn(),
  mutateSkill: vi.fn(),
  renameSkill: vi.fn(),
  openSkillPath: vi.fn(),
}));

const projectSkill: SkillRecord = {
  id: "project-1",
  name: "Project Skill",
  scope: "project",
  workspace: "/tmp/workspace",
  canonical_source_path: "/tmp/workspace/.claude/skills/project-skill",
  target_paths: ["/tmp/workspace/.claude/skills/project-skill"],
  status: "active",
  package_type: "dir",
  skill_key: "project-skill",
};

const globalSkill: SkillRecord = {
  id: "global-1",
  name: "Global Skill",
  scope: "global",
  workspace: null,
  canonical_source_path: "/tmp/home/.claude/skills/global-skill",
  target_paths: ["/tmp/home/.claude/skills/global-skill"],
  status: "active",
  package_type: "dir",
  skill_key: "global-skill",
};

const archivedSkill: SkillRecord = {
  id: "archived-1",
  name: "Archived Skill",
  scope: "global",
  workspace: null,
  canonical_source_path: "/tmp/runtime/archives/abc/source",
  target_paths: ["/tmp/home/.agents/skills/archived-skill"],
  status: "archived",
  package_type: "dir",
  skill_key: "archived-skill",
};

function buildState(skills: SkillRecord[]): SyncState {
  return {
    generated_at: "2026-02-20T17:00:00Z",
    sync: { status: "ok", error: null },
    summary: {
      global_count: skills.filter(
        (skill) => skill.scope === "global" && skill.status === "active",
      ).length,
      project_count: skills.filter(
        (skill) => skill.scope === "project" && skill.status === "active",
      ).length,
      conflict_count: 0,
    },
    skills,
  };
}

function buildDetails(
  skill: SkillRecord,
  overrides?: Partial<SkillDetails>,
): SkillDetails {
  return {
    skill,
    main_file_path: `${skill.canonical_source_path}/SKILL.md`,
    main_file_exists: true,
    main_file_body_preview: "# Preview",
    main_file_body_preview_truncated: false,
    skill_dir_tree_preview: `${skill.skill_key}/\n\`-- SKILL.md`,
    skill_dir_tree_preview_truncated: false,
    last_modified_unix_seconds: 1_700_000_000,
    ...overrides,
  };
}

function setApiDefaults(
  state: SyncState,
  detailsBySkillKey: Record<string, SkillDetails>,
) {
  vi.mocked(tauriApi.getState).mockResolvedValue(state);
  vi.mocked(tauriApi.runSync).mockResolvedValue(state);
  vi.mocked(tauriApi.mutateSkill).mockResolvedValue(state);
  vi.mocked(tauriApi.renameSkill).mockResolvedValue(state);
  vi.mocked(tauriApi.openSkillPath).mockResolvedValue(undefined);
  vi.mocked(tauriApi.getSkillDetails).mockImplementation((skillKey) => {
    const details = detailsBySkillKey[skillKey];
    if (!details) {
      return Promise.reject(new Error(`missing details for ${skillKey}`));
    }
    return Promise.resolve(details);
  });
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.spyOn(window, "confirm").mockReturnValue(true);
});

describe("App critical actions", () => {
  it("uses independent desktop scroll containers for left and right columns", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });

    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    const main = screen.getByRole("main");
    expect(main).toHaveClass("lg:min-h-0");
    expect(main).toHaveClass("lg:flex-1");

    const skillsCard = screen
      .getByRole("heading", { name: "Skills" })
      .closest(".rounded-lg");
    expect(skillsCard).not.toBeNull();
    expect(skillsCard).toHaveClass("lg:flex");
    expect(skillsCard).toHaveClass("lg:flex-col");
    expect(skillsCard).toHaveClass("lg:min-h-0");
    expect(skillsCard).toHaveClass("lg:h-full");
    if (!(skillsCard instanceof HTMLElement)) {
      throw new Error("Skills card must be an HTMLElement.");
    }

    const skillsList = within(skillsCard).getByRole("list");
    const skillsScroller = skillsList.parentElement;
    expect(skillsScroller).not.toBeNull();
    expect(skillsScroller).toHaveClass("lg:flex-1");
    expect(skillsScroller).toHaveClass("lg:min-h-0");
    expect(skillsScroller).toHaveClass("lg:overflow-y-auto");
    expect(skillsScroller).not.toHaveClass("h-[calc(100%-52px)]");

    const detailsCard = screen
      .getByRole("heading", { name: projectSkill.name })
      .closest(".rounded-lg");
    expect(detailsCard).not.toBeNull();
    expect(detailsCard).toHaveClass("lg:flex");
    expect(detailsCard).toHaveClass("lg:flex-col");
    expect(detailsCard).toHaveClass("lg:min-h-0");
    expect(detailsCard).toHaveClass("lg:h-full");
    if (!(detailsCard instanceof HTMLElement)) {
      throw new Error("Details card must be an HTMLElement.");
    }

    const workspaceLabel = within(detailsCard).getByText("Workspace");
    const detailsScroller = workspaceLabel.closest("dl")?.parentElement;
    expect(detailsScroller).not.toBeNull();
    expect(detailsScroller).toHaveClass("lg:flex-1");
    expect(detailsScroller).toHaveClass("lg:min-h-0");
    expect(detailsScroller).toHaveClass("lg:overflow-y-auto");
  });

  it("loads initial state and selected skill details", async () => {
    const state = buildState([projectSkill, archivedSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
      [archivedSkill.skill_key]: buildDetails(archivedSkill),
    });

    render(<App />);

    await screen.findByRole("heading", { name: projectSkill.name });
    expect(tauriApi.getState).toHaveBeenCalledTimes(1);
    expect(tauriApi.getSkillDetails).toHaveBeenCalledWith(
      projectSkill.skill_key,
    );
  });

  it("runs sync and refresh from toolbar", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.click(screen.getByRole("button", { name: "Sync" }));
    await user.click(screen.getByRole("button", { name: "Refresh" }));

    expect(tauriApi.runSync).toHaveBeenCalledTimes(1);
    expect(tauriApi.getState).toHaveBeenCalledTimes(2);
  });

  it("archives skill via in-app confirmation flow", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });
    const confirmSpy = vi.spyOn(window, "confirm");

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.click(screen.getByRole("button", { name: "Archive" }));
    expect(tauriApi.mutateSkill).not.toHaveBeenCalled();
    expect(
      screen.getByText(`Confirm archive_skill for ${projectSkill.skill_key}?`),
    ).toBeInTheDocument();
    expect(confirmSpy).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Confirm action" }));

    expect(tauriApi.mutateSkill).toHaveBeenCalledWith(
      "archive_skill",
      projectSkill.skill_key,
    );
  });

  it("does not mutate when confirmation is rejected", async () => {
    const state = buildState([globalSkill]);
    setApiDefaults(state, {
      [globalSkill.skill_key]: buildDetails(globalSkill),
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: globalSkill.name });

    await user.click(screen.getByRole("button", { name: "Delete" }));
    expect(
      screen.getByText(`Confirm delete_skill for ${globalSkill.skill_key}?`),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Cancel" }));

    expect(tauriApi.mutateSkill).not.toHaveBeenCalled();
  });

  it("restores archived skill", async () => {
    const state = buildState([archivedSkill]);
    setApiDefaults(state, {
      [archivedSkill.skill_key]: buildDetails(archivedSkill),
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: archivedSkill.name });

    await user.click(screen.getByRole("button", { name: "Restore" }));
    await user.click(screen.getByRole("button", { name: "Confirm action" }));

    expect(tauriApi.mutateSkill).toHaveBeenCalledWith(
      "restore_skill",
      archivedSkill.skill_key,
    );
  });

  it("calls make_global and delete for active project skill", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.click(screen.getByRole("button", { name: "Make global" }));
    await user.click(screen.getByRole("button", { name: "Confirm action" }));
    await user.click(screen.getByRole("button", { name: "Delete" }));
    await user.click(screen.getByRole("button", { name: "Confirm action" }));

    expect(tauriApi.mutateSkill).toHaveBeenNthCalledWith(
      1,
      "make_global",
      projectSkill.skill_key,
    );
    expect(tauriApi.mutateSkill).toHaveBeenNthCalledWith(
      2,
      "delete_skill",
      projectSkill.skill_key,
    );
  });

  it("opens folder and file targets", async () => {
    const details = buildDetails(projectSkill, { main_file_exists: true });
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: details,
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    await user.click(screen.getByRole("button", { name: "Open folder" }));
    await user.click(screen.getByRole("button", { name: "Open file" }));

    expect(tauriApi.openSkillPath).toHaveBeenNthCalledWith(
      1,
      projectSkill.skill_key,
      "folder",
    );
    expect(tauriApi.openSkillPath).toHaveBeenNthCalledWith(
      2,
      projectSkill.skill_key,
      "file",
    );
  });

  it("disables opening file when there is no main file", async () => {
    const details = buildDetails(projectSkill, { main_file_exists: false });
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: details,
    });

    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    const openFileButton = screen.getByRole("button", { name: "Open file" });
    expect(openFileButton).toBeDisabled();
  });

  it("opens full skill file from truncated preview link", async () => {
    const details = buildDetails(projectSkill, {
      main_file_body_preview: "# Preview",
      main_file_body_preview_truncated: true,
    });
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: details,
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    expect(screen.getByText(/Preview truncated\./)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "watch full" }));

    expect(tauriApi.openSkillPath).toHaveBeenCalledWith(
      projectSkill.skill_key,
      "file",
    );
  });

  it("renders compact skill dir tree and truncation note", async () => {
    const details = buildDetails(projectSkill, {
      skill_dir_tree_preview: "project-skill/\n|-- references/\n`-- SKILL.md",
      skill_dir_tree_preview_truncated: true,
    });
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: details,
    });

    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    expect(screen.getByText("SKILL dir tree")).toBeInTheDocument();
    expect(
      screen.getByText(/project-skill\/[\s\S]*references\/[\s\S]*SKILL\.md/),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Tree preview truncated for performance."),
    ).toBeInTheDocument();
  });

  it("renames skill and trims title", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    const input = screen.getByPlaceholderText("New skill title");
    await user.clear(input);
    await user.type(input, "  New Skill Name  ");
    await user.click(screen.getByRole("button", { name: "Save name" }));

    expect(tauriApi.renameSkill).toHaveBeenCalledWith(
      projectSkill.skill_key,
      "New Skill Name",
    );
  });

  it("shows error for invalid rename key normalization", async () => {
    const state = buildState([projectSkill]);
    setApiDefaults(state, {
      [projectSkill.skill_key]: buildDetails(projectSkill),
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: projectSkill.name });

    const input = screen.getByPlaceholderText("New skill title");
    await user.clear(input);
    await user.type(input, "___");

    const form = input.closest("form");
    expect(form).not.toBeNull();
    fireEvent.submit(form!);

    await waitFor(() => {
      expect(
        screen.getByText("Rename failed: title must produce non-empty key."),
      ).toBeInTheDocument();
    });
    expect(tauriApi.renameSkill).not.toHaveBeenCalled();
  });
});
