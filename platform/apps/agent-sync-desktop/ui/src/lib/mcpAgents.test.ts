import { describe, expect, it } from "vitest";
import type { McpEnabledByAgent } from "../types";
import { getVisibleMcpAgents, splitMcpAgentsByEnabled } from "./mcpAgents";

describe("mcpAgents helpers", () => {
  it("returns codex and claude", () => {
    expect(getVisibleMcpAgents()).toEqual(["codex", "claude"]);
  });

  it("splits agents into enabled and disabled groups", () => {
    const enabledByAgent: McpEnabledByAgent = {
      codex: true,
      claude: false,
      project: true,
    };

    expect(splitMcpAgentsByEnabled(enabledByAgent)).toEqual({
      enabledAgents: ["codex"],
      disabledAgents: ["claude"],
    });
  });

  it("handles all disabled", () => {
    const enabledByAgent: McpEnabledByAgent = {
      codex: false,
      claude: false,
      project: true,
    };

    expect(splitMcpAgentsByEnabled(enabledByAgent)).toEqual({
      enabledAgents: [],
      disabledAgents: ["codex", "claude"],
    });
  });
});
