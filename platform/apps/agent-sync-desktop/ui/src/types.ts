export type SkillLifecycleStatus = "active" | "archived" | "unmanaged";
export type SyncHealthStatus = "ok" | "failed" | "syncing" | "unknown";
export type PlatformOs = "macos" | "windows" | "linux" | "unknown";

export type PlatformContext = {
  os: PlatformOs;
  linux_desktop: string | null;
};

export type SkillRecord = {
  id: string;
  name: string;
  scope: string;
  workspace: string | null;
  canonical_source_path: string;
  target_paths: string[];
  status: SkillLifecycleStatus;
  package_type: string;
  skill_key: string;
  source?: string | null;
  commit?: string | null;
  install_status?: "ok" | "modified" | "missing" | "unlocked" | null;
  wildcard_source?: string | null;
};

export type DotagentsScope = "all" | "user" | "project";

export type SubagentRecord = {
  id: string;
  name: string;
  description: string;
  scope: string;
  workspace: string | null;
  canonical_source_path: string;
  target_paths: string[];
  exists: boolean;
  is_symlink_canonical: boolean;
  package_type: string;
  subagent_key: string;
  symlink_target: string;
  model: string | null;
  tools: string[];
  codex_tools_ignored: boolean;
  status?: SkillLifecycleStatus;
  archived_at?: string | null;
  archived_bundle_path?: string | null;
  archived_original_scope?: string | null;
  archived_original_workspace?: string | null;
};

export type RuntimeControls = {
  allow_filesystem_changes: boolean;
  auto_watch_active: boolean;
};

export type FocusKind = "skills" | "subagents" | "mcp" | "agents";

export type AuditEventStatus = "success" | "failed" | "blocked";

export type AuditEvent = {
  id: string;
  occurred_at: string;
  action: string;
  status: AuditEventStatus;
  trigger: string | null;
  summary: string;
  paths: string[];
  details: string | null;
};

export type AuditQuery = {
  limit?: number;
  status?: AuditEventStatus;
  action?: string;
};

export type SyncSummary = {
  global_count: number;
  project_count: number;
  conflict_count: number;
  mcp_count: number;
  mcp_warning_count: number;
};

export type SyncMetadata = {
  status: SyncHealthStatus;
  error: string | null;
  warnings?: string[];
};

export type McpEnabledByAgent = {
  codex: boolean;
  claude: boolean;
  project: boolean;
};

export type McpTransport = "stdio" | "http";

export type McpServerRecord = {
  server_key: string;
  scope: "global" | "project";
  workspace: string | null;
  transport: McpTransport;
  command: string | null;
  args: string[];
  url: string | null;
  env: Record<string, string>;
  enabled_by_agent: McpEnabledByAgent;
  targets: string[];
  warnings: string[];
  status?: SkillLifecycleStatus;
  archived_at?: string | null;
};

export type ConfigFormat = "toml" | "json";

export type ConfigValidationResult = {
  path: string;
  format: ConfigFormat;
  valid_syntax: boolean;
  syntax_error?: string | null;
  duplicate_keys: string[];
  warnings: string[];
};

export type SyncState = {
  version?: number;
  generated_at: string;
  sync: SyncMetadata;
  summary: SyncSummary;
  subagent_summary: SyncSummary;
  skills: SkillRecord[];
  subagents: SubagentRecord[];
  mcp_servers?: McpServerRecord[];
  top_skills?: string[];
  top_subagents?: string[];
  config_validations?: ConfigValidationResult[];
};

export type AgentContextSeverity = "ok" | "warning" | "critical";

export type AgentContextSegment = {
  path: string;
  depth: number;
  chars: number;
  lines: number;
  tokens_estimate: number;
};

export type AgentContextEntry = {
  id: string;
  scope: "global" | "project";
  workspace: string | null;
  root_path: string;
  exists: boolean;
  severity: AgentContextSeverity;
  raw_chars: number;
  raw_lines: number;
  rendered_chars: number;
  rendered_lines: number;
  tokens_estimate: number;
  include_count: number;
  missing_includes: string[];
  cycles_detected: string[];
  max_depth_reached: boolean;
  diagnostics: string[];
  segments: AgentContextSegment[];
};

export type AgentsContextLimits = {
  include_max_depth: number;
  file_warning_tokens: number;
  file_critical_tokens: number;
  total_warning_tokens: number;
  total_critical_tokens: number;
  tokens_formula: string;
};

export type AgentsContextTotals = {
  roots_count: number;
  rendered_chars: number;
  rendered_lines: number;
  tokens_estimate: number;
  include_count: number;
  missing_include_count: number;
  cycle_count: number;
  max_depth_reached_count: number;
  severity: AgentContextSeverity;
};

export type AgentsContextReport = {
  generated_at: string;
  limits: AgentsContextLimits;
  totals: AgentsContextTotals;
  warning_count: number;
  critical_count: number;
  entries: AgentContextEntry[];
};

export type DashboardSnapshot = {
  state: SyncState;
  starredSkillIds: string[];
  subagents: SubagentRecord[];
  agentsReport: AgentsContextReport | null;
};

export type SkillDetails = {
  skill: SkillRecord;
  main_file_path: string;
  main_file_exists: boolean;
  main_file_body_preview: string | null;
  skill_dir_tree_preview: string | null;
  last_modified_unix_seconds: number | null;
};

export type RenameSkillResult = {
  state: SyncState;
  renamed_skill_key: string;
};

export type CatalogMutationAction =
  | "archive"
  | "restore"
  | "delete"
  | "make_global";

export type CatalogMutationTarget =
  | { kind: "skill"; skillKey: string }
  | { kind: "subagent"; subagentId: string }
  | {
      kind: "mcp";
      serverKey: string;
      scope: "global" | "project";
      workspace?: string | null;
    };

export type CatalogMutationRequest = {
  action: CatalogMutationAction;
  target: CatalogMutationTarget;
  confirmed: boolean;
};

export type SubagentDetails = {
  subagent: SubagentRecord;
  main_file_path: string;
  main_file_exists: boolean;
  main_file_body_preview: string | null;
  last_modified_unix_seconds: number | null;
};
