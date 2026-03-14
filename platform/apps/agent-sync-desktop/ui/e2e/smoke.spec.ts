import { test, expect } from "./fixtures";

test.describe("Smoke tests", () => {
  test("app loads with runtime and user scope", async ({ tauriPage: page }) => {
    await expect(page.locator("h1")).toHaveText("Dotagents Desktop");
    await expect(page.getByText("Bundled runtime ready")).toBeVisible();
    await expect(page.getByText("User scope")).toBeVisible();
  });

  test("skills tab renders vendor list rows", async ({ tauriPage: page }) => {
    await expect(page.getByText("lint")).toBeVisible();
    await expect(page.getByText("shared")).toBeVisible();
    await expect(page.getByText("Managed by wildcard source")).toBeVisible();
  });

  test("project scope requires an explicit folder", async ({
    tauriPage: page,
  }) => {
    await page.getByRole("button", { name: "Project" }).click();
    await expect(
      page.getByRole("heading", { name: "Choose a project folder" }),
    ).toBeVisible();
  });

  test("output tab shows the last command transcript", async ({
    tauriPage: page,
  }) => {
    await page.getByRole("button", { name: "Sync" }).click();
    await page.getByRole("button", { name: "Output" }).click();
    await expect(page.getByText("dotagents sync --user")).toBeVisible();
    await expect(page.getByText("done")).toBeVisible();
  });
});
