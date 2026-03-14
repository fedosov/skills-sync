export type DotagentsScope = "project" | "user";

export type DotagentsSkillStatus = "ok" | "modified" | "missing" | "unlocked";

export type DotagentsSkillListItem = {
  name: string;
  source: string;
  status: DotagentsSkillStatus;
  commit?: string | null;
  wildcard?: string | null;
};

export type DotagentsMcpTransport = "stdio" | "http";

export type DotagentsMcpListItem = {
  name: string;
  transport: DotagentsMcpTransport;
  target: string;
  env: string[];
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
  actualVersion?: string | null;
  binaryPath?: string | null;
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
