import { test, expect } from "./fixtures";

test.describe("Smoke tests", () => {
  test("app loads and shows header with sync status", async ({
    tauriPage: page,
  }) => {
    await expect(page.locator("h1")).toHaveText("Agent Sync");
    await expect(page.getByText("Ok")).toBeVisible();
  });

  test("header shows skill and MCP counts", async ({ tauriPage: page }) => {
    // Summary line: "Active 2 · Archived 0 · Skills 2 · Subagents 1 · MCP N"
    const summary = page.locator("header p").first();
    await expect(summary).toContainText("Skills");
    await expect(summary).toContainText("MCP");
  });

  test("search input is visible and functional", async ({
    tauriPage: page,
  }) => {
    const search = page.getByPlaceholder(
      "Search by name, key, scope or workspace",
    );
    await expect(search).toBeVisible();
    await search.fill("global");
    await expect(search).toHaveValue("global");
  });

  test("skills are listed in catalog", async ({ tauriPage: page }) => {
    await expect(page.getByText("Global Skill")).toBeVisible();
    await expect(page.getByText("Project Skill")).toBeVisible();
  });

  test("audit log button opens dialog", async ({ tauriPage: page }) => {
    await page.getByRole("button", { name: "Audit log" }).click();
    // The audit log dialog heading should appear
    await expect(
      page.getByRole("heading", { name: "Audit log" }),
    ).toBeVisible();
  });

  test("filesystem toggle shows read-only message by default", async ({
    tauriPage: page,
  }) => {
    // Default: allow_filesystem_changes = false
    await expect(page.getByText("Read-only mode")).toBeVisible();
  });

  test("sync button is disabled in read-only mode", async ({
    tauriPage: page,
  }) => {
    const syncButton = page.getByRole("button", { name: "Sync" });
    await expect(syncButton).toBeDisabled();
  });
});
