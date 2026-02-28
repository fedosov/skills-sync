import { describe, expect, it } from "vitest";
import type { McpEnabledByAgent } from "../types";
import { getVisibleMcpAgents, splitMcpAgentsByEnabled } from "./mcpAgents";

describe("mcpAgents helpers", () => {
  it("returns visible agents by scope", () => {
    expect(getVisibleMcpAgents("global")).toEqual(["codex", "claude"]);
    expect(getVisibleMcpAgents("project")).toEqual([
      "codex",
      "claude",
      "project",
    ]);
  });

  it("splits project scope agents into enabled and disabled groups", () => {
    const enabledByAgent: McpEnabledByAgent = {
      codex: true,
      claude: false,
      project: true,
    };

    expect(splitMcpAgentsByEnabled("project", enabledByAgent)).toEqual({
      enabledAgents: ["codex", "project"],
      disabledAgents: ["claude"],
    });
  });

  it("ignores project agent for global scope", () => {
    const enabledByAgent: McpEnabledByAgent = {
      codex: false,
      claude: true,
      project: true,
    };

    expect(splitMcpAgentsByEnabled("global", enabledByAgent)).toEqual({
      enabledAgents: ["claude"],
      disabledAgents: ["codex"],
    });
  });
});
