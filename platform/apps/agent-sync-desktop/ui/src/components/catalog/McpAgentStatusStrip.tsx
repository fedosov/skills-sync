import { splitMcpAgentsByEnabled, type McpAgentId } from "../../lib/mcpAgents";
import { cn } from "../../lib/utils";
import type { McpEnabledByAgent, McpServerRecord } from "../../types";
import { AgentLogoIcon } from "./AgentLogoIcon";

type McpAgentStatusStripProps = {
  scope: McpServerRecord["scope"];
  enabledByAgent: McpEnabledByAgent;
  className?: string;
};

function AgentStatus({
  agent,
  enabled,
}: {
  agent: McpAgentId;
  enabled: boolean;
}) {
  return (
    <li>
      <span
        role="img"
        aria-label={`${agent} ${enabled ? "enabled" : "disabled"}`}
        className={cn(
          "inline-flex items-center gap-1 rounded-sm px-1 py-0.5 text-[9px] font-semibold uppercase tracking-wide",
          enabled
            ? "text-emerald-500"
            : "text-muted-foreground/75 opacity-60 saturate-50",
        )}
      >
        <AgentLogoIcon agent={agent} className="h-2.5 w-2.5" />
        <span>{enabled ? "ON" : "OFF"}</span>
      </span>
    </li>
  );
}

export function McpAgentStatusStrip({
  scope,
  enabledByAgent,
  className,
}: McpAgentStatusStripProps) {
  const { enabledAgents, disabledAgents } = splitMcpAgentsByEnabled(
    scope,
    enabledByAgent,
  );
  const agents = [
    ...enabledAgents.map((agent) => ({ agent, enabled: true })),
    ...disabledAgents.map((agent) => ({ agent, enabled: false })),
  ];

  return (
    <ul
      className={cn("flex flex-wrap items-center justify-end gap-1", className)}
    >
      {agents.map(({ agent, enabled }) => (
        <AgentStatus key={agent} agent={agent} enabled={enabled} />
      ))}
    </ul>
  );
}
