export type DotagentsScope = "project" | "user";

export type DotagentsSkillStatus = "ok" | "modified" | "missing" | "unlocked";

export type DotagentsSkillListItem = {
  name: string;
  source: string;
  status: DotagentsSkillStatus;
  description?: string | null;
  commit?: string | null;
  wildcard?: string | null;
};

export type DotagentsMcpTransport = "stdio" | "http";

export type DotagentsMcpListItem = {
  name: string;
  transport: DotagentsMcpTransport;
  target: string;
  env: string[];
  description?: string | null;
};

export type DotagentsCommandResult = {
  success: boolean;
  command: string;
  cwd: string;
  scope: DotagentsScope;
  exitCode: number | null;
  durationMs: number;
  stdout: string;
  stderr: string;
};

export type DotagentsRuntimeStatus = {
  available: boolean;
  expectedVersion: string;
  error?: string | null;
};

export type ActiveProjectContext = {
  mode: DotagentsScope;
  projectRoot: string | null;
};

export type AppContext = {
  activeProjectContext: ActiveProjectContext;
  userHome: string;
  userAgentsDir: string;
  userAgentsTomlPath: string;
  userInitialized: boolean;
  projectAgentsTomlPath?: string | null;
  projectInitialized?: boolean | null;
};

export type DotagentsCommandRequest =
  | { kind: "install"; frozen: boolean }
  | { kind: "sync" }
  | { kind: "skillAdd"; source: string; name?: string | null; all: boolean }
  | { kind: "skillRemove"; name: string }
  | { kind: "skillUpdate"; name?: string | null }
  | {
      kind: "mcpAddStdio";
      name: string;
      command: string;
      args: string[];
      env: string[];
    }
  | {
      kind: "mcpAddHttp";
      name: string;
      url: string;
      headers: string[];
      env: string[];
    }
  | { kind: "mcpRemove"; name: string };

// ---------------------------------------------------------------------------
// Skills Workspace
// ---------------------------------------------------------------------------

export type SkillsCliScope = "global" | "project";

export type SkillsCliListItem = {
  name: string;
  path: string;
  scope: SkillsCliScope;
  agents: string[];
  source?: string | null;
  version?: string | null;
  description?: string | null;
};

export type SkillsCliCommandRequest =
  | {
      kind: "add";
      source: string;
      agents: string[];
      scope: SkillsCliScope;
    }
  | {
      kind: "remove";
      name: string;
      agents: string[];
      scope: SkillsCliScope;
    }
  | {
      kind: "update";
      names: string[];
      scope: SkillsCliScope;
    }
  | {
      kind: "restoreLock";
      scope: SkillsCliScope;
    };

export type SkillsCliCommandResult = {
  success: boolean;
  command: string;
  cwd: string;
  scope: SkillsCliScope;
  agents: string[];
  exitCode: number | null;
  durationMs: number;
  stdout: string;
  stderr: string;
};

export type SkillsRuntimeStatus = {
  available: boolean;
  expectedVersion: string;
  error?: string | null;
};

export type SkillsWorkspaceState = {
  scope: SkillsCliScope;
  activeAgents: string[];
  versionOverride?: string | null;
  initialized: boolean;
};

export type SkillsWorkspaceContext = {
  state: SkillsWorkspaceState;
  detectedAgents: string[];
  runtimeStatus: SkillsRuntimeStatus;
};

export type WorkspaceKey = "dotagents" | "skills";
