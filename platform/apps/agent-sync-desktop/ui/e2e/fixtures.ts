import { test as base, type Page } from "@playwright/test";

async function injectTauriMock(page: Page) {
  await page.addInitScript(() => {
    let currentContext = {
      activeProjectContext: {
        mode: "user",
        projectRoot: null,
      },
      userHome: "/Users/tester",
      userAgentsDir: "/Users/tester/.agents",
      userAgentsTomlPath: "/Users/tester/.agents/agents.toml",
      userInitialized: true,
      projectAgentsTomlPath: null,
      projectInitialized: null,
    };

    const runtimeStatus = {
      available: true,
      expectedVersion: "1.4.0",
    };

    const skills = [
      {
        name: "lint",
        source: "owner/repo",
        status: "ok",
        commit: "deadbeef",
      },
      {
        name: "shared",
        source: "owner/repo",
        status: "ok",
        wildcard: "owner/repo",
      },
    ];

    const mcpServers = [
      {
        name: "github",
        transport: "stdio",
        target: "npx",
        env: ["GITHUB_TOKEN"],
      },
    ];

    function clone<T>(value: T): T {
      return JSON.parse(JSON.stringify(value)) as T;
    }

    (window as never as Record<string, unknown>).__TAURI_INTERNALS__ = {
      invoke: (cmd: string, args?: Record<string, unknown>) => {
        switch (cmd) {
          case "get_runtime_status":
            return Promise.resolve(clone(runtimeStatus));
          case "get_app_context":
            return Promise.resolve(clone(currentContext));
          case "set_scope": {
            currentContext =
              args?.scope === "project"
                ? clone({
                    ...currentContext,
                    activeProjectContext: {
                      mode: "project",
                      projectRoot: null,
                    },
                    projectAgentsTomlPath: null,
                    projectInitialized: null,
                  })
                : clone({
                    ...currentContext,
                    activeProjectContext: {
                      mode: "user",
                      projectRoot: null,
                    },
                    projectAgentsTomlPath: null,
                    projectInitialized: null,
                  });
            return Promise.resolve(clone(currentContext));
          }
          case "set_project_root": {
            const projectRoot =
              typeof args?.projectRoot === "string" ? args.projectRoot : null;
            currentContext = clone({
              ...currentContext,
              activeProjectContext: {
                mode: "project",
                projectRoot,
              },
              projectAgentsTomlPath: projectRoot
                ? `${projectRoot}/agents.toml`
                : null,
              projectInitialized: projectRoot ? true : null,
            });
            return Promise.resolve(clone(currentContext));
          }
          case "list_skills":
            return Promise.resolve(clone(skills));
          case "list_mcp_servers":
            return Promise.resolve(clone(mcpServers));
          case "run_dotagents_command": {
            const request = (args?.request ?? {}) as Record<string, unknown>;
            const scope = currentContext.activeProjectContext.mode;
            const cwd =
              scope === "project"
                ? (currentContext.activeProjectContext.projectRoot ??
                  "/tmp/workspace")
                : "/Users/tester";
            const command =
              request.kind === "sync"
                ? scope === "user"
                  ? "dotagents sync --user"
                  : "dotagents sync"
                : "dotagents command";

            return Promise.resolve({
              success: true,
              command,
              cwd,
              scope,
              exitCode: 0,
              durationMs: 28,
              stdout: "done",
              stderr: "",
            });
          }
          case "open_agents_toml":
          case "open_agents_dir":
          case "open_user_home":
            return Promise.resolve(null);
          default:
            console.warn(`[e2e] unhandled invoke: ${cmd}`);
            return Promise.resolve(null);
        }
      },
      metadata: { currentWebview: { label: "main" } },
    };
  });
}

export const test = base.extend<{ tauriPage: Page }>({
  tauriPage: async ({ page }, use) => {
    await injectTauriMock(page);
    await page.goto("/");
    await use(page);
  },
});

export { expect } from "@playwright/test";
