import type {
  AgentContextEntry,
  AgentContextSeverity,
  AuditEventStatus,
  CatalogMutationTarget,
  FocusKind,
  McpServerRecord,
  SkillLifecycleStatus,
  SubagentRecord,
  SyncHealthStatus,
} from "../types";
import { cn } from "./utils";

export function toTitleCase(value: string): string {
  if (!value) {
    return value;
  }
  return `${value.charAt(0).toUpperCase()}${value.slice(1)}`;
}

export function subagentStatus(subagent: SubagentRecord): SkillLifecycleStatus {
  return subagent.status ?? "active";
}

export function mcpStatus(server: McpServerRecord): SkillLifecycleStatus {
  return server.status ?? "active";
}

export function statusRank(status: SkillLifecycleStatus): number {
  if (status === "active") return 0;
  if (status === "unmanaged") return 1;
  return 2;
}

export function syncStatusVariant(status: SyncHealthStatus | undefined) {
  switch (status) {
    case "ok":
      return "success" as const;
    case "failed":
      return "error" as const;
    case "syncing":
      return "warning" as const;
    default:
      return "outline" as const;
  }
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

export function warningMentionsServer(
  warning: string,
  serverKey: string,
): boolean {
  const escaped = escapeRegExp(serverKey);
  const catalogIdPattern = new RegExp(`::${escaped}(?=$|[^A-Za-z0-9_-])`);
  return (
    warning.includes(`'${serverKey}'`) ||
    warning.includes(`"${serverKey}"`) ||
    catalogIdPattern.test(warning)
  );
}

export function syncWarningFixSummary(warning: string): string | null {
  if (warning.startsWith("Broken unmanaged Claude MCP '")) {
    return "Will remove broken unmanaged Claude entry";
  }
  if (
    warning.startsWith("MCP server '") &&
    warning.includes(" exists in ") &&
    warning.endsWith(" but is unmanaged in central catalog")
  ) {
    return "Will add server to managed MCP list";
  }
  if (
    warning.startsWith("MCP server '") &&
    warning.includes("' has inline secret-like env value for '") &&
    warning.endsWith("'")
  ) {
    return "Will replace inline secret with env variable (env must be set first)";
  }
  if (
    warning.startsWith("MCP server '") &&
    warning.includes("' has inline secret-like argument '") &&
    warning.endsWith("'")
  ) {
    return "Will replace secret argument with env variable (env must be set first)";
  }
  if (
    warning.startsWith("Skipped managed Codex MCP '") &&
    warning.includes("' because unmanaged entry already exists in ")
  ) {
    return "Will remove duplicate unmanaged Codex entry";
  }
  if (
    warning.startsWith("Skipped project MCP target ") &&
    warning.endsWith(" because file does not exist")
  ) {
    return "Will create missing project MCP file";
  }
  return null;
}

export function isFixableSyncWarning(warning: string): boolean {
  return syncWarningFixSummary(warning) !== null;
}

export type AuditStatusFilter = AuditEventStatus | "all";

export function parseAuditStatusFilter(value: string): AuditStatusFilter {
  switch (value) {
    case "success":
    case "failed":
    case "blocked":
    case "all":
      return value;
    default:
      return "all";
  }
}

function parseFocusKind(value: string | null): FocusKind {
  if (
    value === "skills" ||
    value === "subagents" ||
    value === "mcp" ||
    value === "agents"
  ) {
    return value;
  }
  return "skills";
}

export function severityRank(severity: AgentContextSeverity): number {
  if (severity === "critical") return 2;
  if (severity === "warning") return 1;
  return 0;
}

export function severityDotClass(severity: AgentContextSeverity): string {
  return cn(
    "inline-block h-2 w-2 rounded-full",
    severity === "critical"
      ? "bg-red-500"
      : severity === "warning"
        ? "bg-amber-500"
        : "bg-emerald-500",
  );
}

export const CATALOG_FOCUS_STORAGE_KEY = "agent-sync.catalog.focusKind.v1";

export function readStoredFocusKind(): FocusKind {
  if (typeof window === "undefined") {
    return "skills";
  }
  try {
    return parseFocusKind(
      window.localStorage.getItem(CATALOG_FOCUS_STORAGE_KEY),
    );
  } catch {
    return "skills";
  }
}

export function mcpTarget(server: McpServerRecord): CatalogMutationTarget {
  return {
    kind: "mcp",
    serverKey: server.server_key,
    scope: server.scope,
    workspace: server.workspace,
  };
}

export function mcpDeleteLabel(server: McpServerRecord): string {
  const status = mcpStatus(server);
  if (status === "unmanaged") {
    return `unmanaged MCP server "${server.server_key}"`;
  }
  if (server.scope === "project") {
    return `MCP server "${server.server_key}" (Project: ${server.workspace ?? "unknown workspace"})`;
  }
  return `MCP server "${server.server_key}" (Global)`;
}

export function mcpSelectionKey(server: McpServerRecord): string {
  return `${server.scope}::${server.workspace ?? "global"}::${server.server_key}`;
}

export function sortAndFilter<T>(
  items: T[],
  query: string,
  compareFn: (a: T, b: T) => number,
  searchFields: (item: T) => string[],
): T[] {
  const ordered = items.slice().sort(compareFn);
  const q = query.trim().toLowerCase();
  if (!q) return ordered;
  return ordered.filter((item) =>
    searchFields(item).some((field) => field.toLowerCase().includes(q)),
  );
}

function favoriteRank(favorites: Set<string>, key: string): number {
  return favorites.has(key) ? 0 : 1;
}

function compareNullableText(
  lhs: string | null | undefined,
  rhs: string | null | undefined,
): number {
  return (lhs ?? "").localeCompare(rhs ?? "");
}

export function sortAndFilterSubagents(
  subagents: SubagentRecord[],
  query: string,
  favoriteIds: Set<string>,
): SubagentRecord[] {
  return sortAndFilter(
    subagents,
    query,
    (lhs, rhs) =>
      statusRank(subagentStatus(lhs)) - statusRank(subagentStatus(rhs)) ||
      favoriteRank(favoriteIds, lhs.id) - favoriteRank(favoriteIds, rhs.id) ||
      lhs.name.localeCompare(rhs.name) ||
      lhs.scope.localeCompare(rhs.scope) ||
      compareNullableText(lhs.workspace, rhs.workspace),
    (item) => [
      item.name,
      item.subagent_key,
      item.scope,
      item.workspace ?? "",
      item.description,
    ],
  );
}

export function sortAndFilterMcpServers(
  servers: McpServerRecord[],
  query: string,
  favoriteIds: Set<string>,
): McpServerRecord[] {
  return sortAndFilter(
    servers,
    query,
    (lhs, rhs) =>
      statusRank(mcpStatus(lhs)) - statusRank(mcpStatus(rhs)) ||
      favoriteRank(favoriteIds, mcpSelectionKey(lhs)) -
        favoriteRank(favoriteIds, mcpSelectionKey(rhs)) ||
      lhs.server_key.localeCompare(rhs.server_key) ||
      lhs.scope.localeCompare(rhs.scope) ||
      compareNullableText(lhs.workspace, rhs.workspace),
    (item) => [
      item.server_key,
      item.scope,
      item.workspace ?? "",
      item.transport,
      item.command ?? "",
      item.url ?? "",
    ],
  );
}

export function sortAndFilterAgentEntries(
  entries: AgentContextEntry[],
  query: string,
  favoriteIds: Set<string>,
): AgentContextEntry[] {
  return sortAndFilter(
    entries,
    query,
    (lhs, rhs) =>
      severityRank(rhs.severity) - severityRank(lhs.severity) ||
      favoriteRank(favoriteIds, lhs.id) - favoriteRank(favoriteIds, rhs.id) ||
      lhs.scope.localeCompare(rhs.scope) ||
      compareNullableText(lhs.workspace, rhs.workspace) ||
      lhs.root_path.localeCompare(rhs.root_path),
    (entry) => [
      entry.root_path,
      entry.scope,
      entry.workspace ?? "",
      entry.severity,
    ],
  );
}
