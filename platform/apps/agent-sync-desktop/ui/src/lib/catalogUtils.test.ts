import { describe, expect, it } from "vitest";
import {
  mcpSelectionKey,
  sortAndFilter,
  sortAndFilterAgentEntries,
  sortAndFilterMcpServers,
  sortAndFilterSubagents,
} from "./catalogUtils";
import type {
  AgentContextEntry,
  McpServerRecord,
  SubagentRecord,
} from "../types";

interface Item {
  name: string;
  rank: number;
  tag: string;
}

const items: Item[] = [
  { name: "Charlie", rank: 2, tag: "b" },
  { name: "Alpha", rank: 1, tag: "a" },
  { name: "Bravo", rank: 1, tag: "b" },
];

const compareFn = (a: Item, b: Item) =>
  a.rank - b.rank || a.name.localeCompare(b.name);

const searchFields = (item: Item) => [item.name, item.tag];

function makeSubagent(overrides: Partial<SubagentRecord> = {}): SubagentRecord {
  return {
    id: "subagent-id",
    name: "Default Subagent",
    description: "Default description",
    scope: "global",
    workspace: null,
    canonical_source_path: "/tmp/subagent",
    target_paths: [],
    exists: true,
    is_symlink_canonical: true,
    package_type: "dir",
    subagent_key: "default-subagent",
    symlink_target: "/tmp/subagent",
    model: null,
    tools: [],
    codex_tools_ignored: false,
    status: "active",
    archived_at: null,
    archived_bundle_path: null,
    archived_original_scope: null,
    archived_original_workspace: null,
    ...overrides,
  };
}

function makeMcpServer(
  overrides: Partial<McpServerRecord> = {},
): McpServerRecord {
  return {
    server_key: "default-server",
    scope: "global",
    workspace: null,
    transport: "stdio",
    command: "node",
    args: [],
    url: null,
    env: {},
    enabled_by_agent: {
      codex: true,
      claude: true,
      project: false,
    },
    targets: [],
    warnings: [],
    status: "active",
    archived_at: null,
    ...overrides,
  };
}

function makeAgentEntry(
  overrides: Partial<AgentContextEntry> = {},
): AgentContextEntry {
  return {
    id: "agent-id",
    scope: "global",
    workspace: null,
    root_path: "/tmp/agent",
    exists: true,
    severity: "ok",
    raw_chars: 10,
    raw_lines: 2,
    rendered_chars: 10,
    rendered_lines: 2,
    tokens_estimate: 10,
    include_count: 0,
    missing_includes: [],
    cycles_detected: [],
    max_depth_reached: false,
    diagnostics: [],
    segments: [],
    ...overrides,
  };
}

describe("sortAndFilter", () => {
  it("sorts items by compareFn when no query", () => {
    const result = sortAndFilter(items, "", compareFn, searchFields);
    expect(result.map((i) => i.name)).toEqual(["Alpha", "Bravo", "Charlie"]);
  });

  it("does not mutate original array", () => {
    const copy = [...items];
    sortAndFilter(items, "", compareFn, searchFields);
    expect(items).toEqual(copy);
  });

  it("filters by query (case-insensitive)", () => {
    const result = sortAndFilter(items, "alpha", compareFn, searchFields);
    expect(result).toHaveLength(1);
    expect(result[0].name).toBe("Alpha");
  });

  it("trims and lowercases query", () => {
    const result = sortAndFilter(items, "  BRAVO  ", compareFn, searchFields);
    expect(result).toHaveLength(1);
    expect(result[0].name).toBe("Bravo");
  });

  it("filters by secondary field (tag)", () => {
    const result = sortAndFilter(items, "a", compareFn, searchFields);
    expect(result.map((i) => i.name)).toEqual(["Alpha", "Bravo", "Charlie"]);
  });

  it("returns empty when nothing matches", () => {
    const result = sortAndFilter(items, "zzz", compareFn, searchFields);
    expect(result).toHaveLength(0);
  });

  it("returns empty for empty input", () => {
    const result = sortAndFilter([], "test", compareFn, searchFields);
    expect(result).toHaveLength(0);
  });
});

describe("mcpSelectionKey", () => {
  it("uses scope, workspace, and server key in a stable identifier", () => {
    expect(
      mcpSelectionKey(
        makeMcpServer({
          scope: "project",
          workspace: "/tmp/repo",
          server_key: "github",
        }),
      ),
    ).toBe("project::/tmp/repo::github");
  });

  it("normalizes missing workspace to global", () => {
    expect(mcpSelectionKey(makeMcpServer({ server_key: "github" }))).toBe(
      "global::global::github",
    );
  });
});

describe("sortAndFilterSubagents", () => {
  it("keeps active favorites first, then sorts by name and workspace", () => {
    const subagents = [
      makeSubagent({
        id: "sub-z",
        name: "Zulu",
        status: "active",
        scope: "project",
        workspace: "/tmp/repo-b",
      }),
      makeSubagent({
        id: "sub-a",
        name: "Alpha",
        status: "active",
        scope: "global",
      }),
      makeSubagent({
        id: "sub-old",
        name: "Legacy",
        status: "archived",
      }),
    ];

    const ordered = sortAndFilterSubagents(subagents, "", new Set(["sub-z"]));

    expect(ordered.map((item) => item.id)).toEqual([
      "sub-z",
      "sub-a",
      "sub-old",
    ]);
  });

  it("filters subagents by description and workspace", () => {
    const subagents = [
      makeSubagent({
        id: "sub-docs",
        description: "Writes release notes",
        workspace: "/tmp/docs",
      }),
      makeSubagent({
        id: "sub-dev",
        description: "Handles runtime errors",
        workspace: "/tmp/runtime",
      }),
    ];

    expect(
      sortAndFilterSubagents(subagents, "release", new Set()).map(
        (item) => item.id,
      ),
    ).toEqual(["sub-docs"]);
    expect(
      sortAndFilterSubagents(subagents, "/tmp/runtime", new Set()).map(
        (item) => item.id,
      ),
    ).toEqual(["sub-dev"]);
  });
});

describe("sortAndFilterMcpServers", () => {
  it("keeps active favorites first, then sorts by key and workspace", () => {
    const servers = [
      makeMcpServer({
        server_key: "zeta",
        scope: "project",
        workspace: "/tmp/repo-b",
      }),
      makeMcpServer({
        server_key: "alpha",
        scope: "global",
      }),
      makeMcpServer({
        server_key: "old",
        status: "archived",
      }),
    ];

    const ordered = sortAndFilterMcpServers(
      servers,
      "",
      new Set([mcpSelectionKey(servers[0])]),
    );

    expect(ordered.map((item) => item.server_key)).toEqual([
      "zeta",
      "alpha",
      "old",
    ]);
  });

  it("filters by server metadata fields", () => {
    const servers = [
      makeMcpServer({
        server_key: "docs",
        command: "uvx",
        args: ["serve"],
        workspace: "/tmp/docs",
      }),
      makeMcpServer({
        server_key: "remote",
        transport: "http",
        url: "https://example.test/mcp",
      }),
    ];

    expect(
      sortAndFilterMcpServers(servers, "uvx", new Set()).map(
        (item) => item.server_key,
      ),
    ).toEqual(["docs"]);
    expect(
      sortAndFilterMcpServers(servers, "example.test", new Set()).map(
        (item) => item.server_key,
      ),
    ).toEqual(["remote"]);
  });
});

describe("sortAndFilterAgentEntries", () => {
  it("sorts critical entries first and keeps favorites ahead within same severity", () => {
    const entries = [
      makeAgentEntry({
        id: "agent-ok",
        severity: "ok",
        root_path: "/tmp/ok",
      }),
      makeAgentEntry({
        id: "agent-warning",
        severity: "warning",
        root_path: "/tmp/warning",
      }),
      makeAgentEntry({
        id: "agent-critical",
        severity: "critical",
        root_path: "/tmp/critical",
      }),
    ];

    const ordered = sortAndFilterAgentEntries(
      entries,
      "",
      new Set(["agent-warning"]),
    );

    expect(ordered.map((item) => item.id)).toEqual([
      "agent-critical",
      "agent-warning",
      "agent-ok",
    ]);
  });

  it("filters by root path and scope", () => {
    const entries = [
      makeAgentEntry({
        id: "agent-global",
        scope: "global",
        root_path: "/tmp/global",
      }),
      makeAgentEntry({
        id: "agent-project",
        scope: "project",
        workspace: "/tmp/repo",
        root_path: "/tmp/repo/.agents",
      }),
    ];

    expect(
      sortAndFilterAgentEntries(entries, "project", new Set()).map(
        (item) => item.id,
      ),
    ).toEqual(["agent-project"]);
    expect(
      sortAndFilterAgentEntries(entries, "/tmp/global", new Set()).map(
        (item) => item.id,
      ),
    ).toEqual(["agent-global"]);
  });
});
