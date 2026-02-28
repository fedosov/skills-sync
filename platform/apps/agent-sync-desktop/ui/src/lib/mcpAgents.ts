import type { McpEnabledByAgent, McpServerRecord } from "../types";

export type McpAgentId = "codex" | "claude" | "project";

const MCP_AGENTS_BY_SCOPE: Record<McpServerRecord["scope"], McpAgentId[]> = {
  global: ["codex", "claude"],
  project: ["codex", "claude", "project"],
};

export function getVisibleMcpAgents(
  scope: McpServerRecord["scope"],
): McpAgentId[] {
  return MCP_AGENTS_BY_SCOPE[scope];
}

export function splitMcpAgentsByEnabled(
  scope: McpServerRecord["scope"],
  enabledByAgent: McpEnabledByAgent,
): {
  enabledAgents: McpAgentId[];
  disabledAgents: McpAgentId[];
} {
  const visibleAgents = getVisibleMcpAgents(scope);
  const enabledAgents = visibleAgents.filter((agent) => enabledByAgent[agent]);
  const disabledAgents = visibleAgents.filter(
    (agent) => !enabledByAgent[agent],
  );

  return { enabledAgents, disabledAgents };
}
