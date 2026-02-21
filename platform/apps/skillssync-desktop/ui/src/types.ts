export type SkillLifecycleStatus = "active" | "archived";
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
};

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
};

export type SkillDetails = {
  skill: SkillRecord;
  main_file_path: string;
  main_file_exists: boolean;
  main_file_body_preview: string | null;
  main_file_body_preview_truncated: boolean;
  skill_dir_tree_preview: string | null;
  skill_dir_tree_preview_truncated: boolean;
  last_modified_unix_seconds: number | null;
};

export type MutationCommand =
  | "archive_skill"
  | "restore_skill"
  | "delete_skill"
  | "make_global";

export type SubagentTargetKind =
  | "symlink"
  | "regular_file"
  | "missing"
  | "other";

export type SubagentTargetStatus = {
  path: string;
  exists: boolean;
  is_symlink: boolean;
  symlink_target: string | null;
  points_to_canonical: boolean;
  kind: SubagentTargetKind;
};

export type SubagentDetails = {
  subagent: SubagentRecord;
  main_file_path: string;
  main_file_exists: boolean;
  main_file_body_preview: string | null;
  main_file_body_preview_truncated: boolean;
  subagent_dir_tree_preview: string | null;
  subagent_dir_tree_preview_truncated: boolean;
  last_modified_unix_seconds: number | null;
  target_statuses: SubagentTargetStatus[];
};
