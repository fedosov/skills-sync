import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { McpDetailsPanel } from "./McpDetailsPanel";
import { SkillDetailsPanel } from "./SkillDetailsPanel";
import { SubagentDetailsPanel } from "./SubagentDetailsPanel";
import type {
  McpServerRecord,
  SkillDetails,
  SubagentDetails,
} from "../../types";

function buildSkillDetails(): SkillDetails {
  return {
    skill: {
      id: "skill-1",
      name: "Project Skill",
      scope: "project",
      workspace: "/tmp/workspace",
      canonical_source_path: "/tmp/workspace/.claude/skills/project-skill",
      target_paths: ["/tmp/workspace/.claude/skills/project-skill"],
      status: "active",
      package_type: "dir",
      skill_key: "project-skill",
    },
    main_file_path: "/tmp/workspace/.claude/skills/project-skill/SKILL.md",
    main_file_exists: false,
    main_file_body_preview: null,
    skill_dir_tree_preview: null,
    last_modified_unix_seconds: null,
  };
}

function buildSubagentDetails(): SubagentDetails {
  return {
    subagent: {
      id: "subagent-1",
      name: "Subagent",
      description: "Helpful subagent",
      scope: "global",
      workspace: null,
      canonical_source_path: "/tmp/home/.claude/agents/subagent.md",
      target_paths: ["/tmp/home/.claude/agents/subagent.md"],
      exists: true,
      is_symlink_canonical: false,
      package_type: "file",
      subagent_key: "subagent",
      symlink_target: "/tmp/home/.claude/agents/subagent.md",
      model: null,
      tools: [],
      codex_tools_ignored: false,
      status: "active",
    },
    main_file_path: "/tmp/home/.claude/agents/subagent.md",
    main_file_exists: true,
    main_file_body_preview: null,
    last_modified_unix_seconds: null,
  };
}

function buildServer(
  overrides: Partial<McpServerRecord> = {},
): McpServerRecord {
  return {
    server_key: "exa",
    scope: "global",
    workspace: null,
    transport: "http",
    command: null,
    args: [],
    url: "https://example.com/mcp",
    env: {},
    enabled_by_agent: {
      codex: true,
      claude: false,
      project: false,
    },
    targets: [],
    warnings: [],
    ...overrides,
  };
}

describe("detail panels", () => {
  it("wires skill details actions through shared menus", async () => {
    const onOpenPath = vi.fn();
    const onArchive = vi.fn();
    const onMakeGlobal = vi.fn();
    const onRequestDelete = vi.fn();

    render(
      <SkillDetailsPanel
        details={buildSkillDetails()}
        busy={false}
        isFavorite={false}
        onToggleFavorite={vi.fn()}
        renameDraft="Project Skill"
        openTargetMenu={true}
        actionsMenuOpen={true}
        onRenameDraftChange={vi.fn()}
        onRenameSubmit={vi.fn()}
        onToggleOpenTargetMenu={vi.fn()}
        onToggleActionsMenu={vi.fn()}
        onOpenPath={onOpenPath}
        onArchive={onArchive}
        onMakeGlobal={onMakeGlobal}
        onRestore={vi.fn()}
        onRequestDelete={onRequestDelete}
        onCopyPath={vi.fn()}
      />,
    );

    expect(screen.getByRole("button", { name: "Open…" })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: "Open file" })).toBeDisabled();

    await userEvent.click(
      screen.getByRole("menuitem", { name: "Open folder" }),
    );
    await userEvent.click(screen.getByRole("menuitem", { name: "Archive" }));
    await userEvent.click(
      screen.getByRole("menuitem", { name: "Make global" }),
    );
    await userEvent.click(screen.getByRole("menuitem", { name: "Delete" }));

    expect(onOpenPath).toHaveBeenCalledWith("folder");
    expect(onArchive).toHaveBeenCalledOnce();
    expect(onMakeGlobal).toHaveBeenCalledOnce();
    expect(onRequestDelete).toHaveBeenCalledOnce();
  });

  it("wires subagent details actions through shared menus", async () => {
    const onOpenPath = vi.fn();
    const onArchive = vi.fn();

    render(
      <SubagentDetailsPanel
        subagentDetails={buildSubagentDetails()}
        busy={false}
        isFavorite={false}
        onToggleFavorite={vi.fn()}
        openTargetMenu={true}
        actionsMenuOpen={true}
        onToggleOpenTargetMenu={vi.fn()}
        onToggleActionsMenu={vi.fn()}
        onOpenPath={onOpenPath}
        onArchive={onArchive}
        onRestore={vi.fn()}
        onRequestDelete={vi.fn()}
      />,
    );

    expect(screen.getByRole("button", { name: "Open…" })).toBeInTheDocument();
    expect(
      screen.queryByRole("menuitem", { name: "Make global" }),
    ).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("menuitem", { name: "Open file" }));
    await userEvent.click(screen.getByRole("menuitem", { name: "Archive" }));

    expect(onOpenPath).toHaveBeenCalledWith("file");
    expect(onArchive).toHaveBeenCalledOnce();
  });

  it("uses actions-only menu for archived mcp servers", async () => {
    const onRestore = vi.fn();
    const onDelete = vi.fn();

    render(
      <McpDetailsPanel
        server={buildServer({ status: "archived" })}
        warnings={[]}
        busy={false}
        fixingWarning={null}
        isFavorite={false}
        onToggleFavorite={vi.fn()}
        actionsMenuOpen={true}
        onToggleActionsMenu={vi.fn()}
        onSetEnabled={vi.fn()}
        onFixWarning={vi.fn()}
        onArchive={vi.fn()}
        onMakeGlobal={vi.fn()}
        onRestore={onRestore}
        onRequestDelete={onDelete}
      />,
    );

    expect(
      screen.queryByRole("button", { name: "Open…" }),
    ).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("menuitem", { name: "Restore" }));
    await userEvent.click(screen.getByRole("menuitem", { name: "Delete" }));

    expect(onRestore).toHaveBeenCalledOnce();
    expect(onDelete).toHaveBeenCalledOnce();
  });
});
