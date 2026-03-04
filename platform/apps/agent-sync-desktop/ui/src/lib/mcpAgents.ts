import type { McpEnabledByAgent } from "../types";

export type McpAgentId = "codex" | "claude";

const VISIBLE_MCP_AGENTS: McpAgentId[] = ["codex", "claude"];

export function getVisibleMcpAgents(): McpAgentId[] {
  return VISIBLE_MCP_AGENTS;
}

export function splitMcpAgentsByEnabled(enabledByAgent: McpEnabledByAgent): {
  enabledAgents: McpAgentId[];
  disabledAgents: McpAgentId[];
} {
  const visibleAgents = getVisibleMcpAgents();
  const enabledAgents = visibleAgents.filter((agent) => enabledByAgent[agent]);
  const disabledAgents = visibleAgents.filter(
    (agent) => !enabledByAgent[agent],
  );

  return { enabledAgents, disabledAgents };
}
