import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { McpListPanel } from "./McpListPanel";
import { SkillListPanel } from "./SkillListPanel";
import { SubagentListPanel } from "./SubagentListPanel";
import type { McpServerRecord, SkillRecord, SubagentRecord } from "../../types";

const emptyGroups = {};

function buildSkill(): SkillRecord {
  return {
    id: "skill-1",
    name: "Skill Alpha",
    scope: "global",
    workspace: null,
    canonical_source_path: "/tmp/home/.agents/skills/skill-alpha",
    target_paths: ["/tmp/home/.agents/skills/skill-alpha"],
    status: "active",
    package_type: "dir",
    skill_key: "skill-alpha",
  };
}

function buildSubagent(): SubagentRecord {
  return {
    id: "subagent-1",
    name: "Subagent Alpha",
    description: "Helpful subagent",
    scope: "global",
    workspace: null,
    canonical_source_path: "/tmp/home/.claude/agents/subagent-alpha.md",
    target_paths: ["/tmp/home/.claude/agents/subagent-alpha.md"],
    exists: true,
    is_symlink_canonical: false,
    package_type: "file",
    subagent_key: "subagent-alpha",
    symlink_target: "/tmp/home/.claude/agents/subagent-alpha.md",
    model: null,
    tools: [],
    codex_tools_ignored: false,
    status: "active",
  };
}

function buildServer(): McpServerRecord {
  return {
    server_key: "exa",
    scope: "project",
    workspace: "/tmp/workspace",
    transport: "http",
    command: null,
    args: [],
    url: "https://example.com/mcp",
    env: {},
    enabled_by_agent: {
      codex: true,
      claude: false,
      project: true,
    },
    targets: ["/tmp/workspace/.mcp.json"],
    warnings: [],
    status: "active",
  };
}

describe("catalog panels", () => {
  it("wires skill row selection through the shared row shell", async () => {
    const onSelect = vi.fn();
    const onCloseMenus = vi.fn();

    render(
      <SkillListPanel
        skills={[buildSkill()]}
        query=""
        selectedSkillKey={null}
        favorites={new Set(["skill-1"])}
        emptyText="No skills"
        expandedProjectGroups={emptyGroups}
        onSelect={onSelect}
        onToggleProjectGroup={vi.fn()}
        onCloseMenus={onCloseMenus}
      />,
    );

    await userEvent.click(screen.getByRole("button", { name: /Skill Alpha/i }));

    expect(onSelect).toHaveBeenCalledWith("skill-alpha");
    expect(onCloseMenus).toHaveBeenCalledOnce();
  });

  it("wires subagent row selection through the shared row shell", async () => {
    const onSelect = vi.fn();
    const onCloseMenus = vi.fn();

    render(
      <SubagentListPanel
        subagents={[buildSubagent()]}
        query=""
        selectedSubagentId={null}
        favorites={new Set(["subagent-1"])}
        emptyText="No subagents"
        expandedProjectGroups={emptyGroups}
        onSelect={onSelect}
        onToggleProjectGroup={vi.fn()}
        onCloseMenus={onCloseMenus}
      />,
    );

    await userEvent.click(
      screen.getByRole("button", { name: /Subagent Alpha/i }),
    );

    expect(onSelect).toHaveBeenCalledWith("subagent-1");
    expect(onCloseMenus).toHaveBeenCalledOnce();
  });

  it("wires mcp row selection through the shared row shell", async () => {
    const onSelect = vi.fn();
    const onCloseMenus = vi.fn();

    render(
      <McpListPanel
        servers={[buildServer()]}
        query=""
        selectedMcpKey={null}
        favorites={new Set(["project::/tmp/workspace::exa"])}
        emptyText="No MCP servers"
        expandedProjectGroups={{ "/tmp/workspace": true }}
        onSelect={onSelect}
        onToggleProjectGroup={vi.fn()}
        onCloseMenus={onCloseMenus}
      />,
    );

    await userEvent.click(screen.getByRole("button", { name: /exa/i }));

    expect(onSelect).toHaveBeenCalledWith("project::/tmp/workspace::exa");
    expect(onCloseMenus).toHaveBeenCalledOnce();
    expect(screen.getByText("HTTP")).toBeInTheDocument();
  });
});
