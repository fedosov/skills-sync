import { test, expect } from "./fixtures";

test.describe("Smoke tests", () => {
  test("app loads with runtime and user scope", async ({ tauriPage: page }) => {
    await expect(page.locator("h1")).toHaveText("Dotagents Desktop");
    await expect(page.getByText("Runtime via npx ready")).toBeVisible();
    await expect(page.getByText("User scope")).toBeVisible();
  });

  test("skills section renders vendor list rows", async ({
    tauriPage: page,
  }) => {
    await expect(page.getByText("lint")).toBeVisible();
    await expect(page.getByText("shared")).toBeVisible();
    await expect(page.getByText(/wildcard/i)).toBeVisible();
  });

  test("project scope requires an explicit folder", async ({
    tauriPage: page,
  }) => {
    await page.getByRole("button", { name: "Project" }).click();
    await expect(
      page.getByRole("heading", { name: "Choose a project folder" }),
    ).toBeVisible();
  });

  test("output section shows the last command transcript", async ({
    tauriPage: page,
  }) => {
    await page.getByRole("button", { name: "Sync", exact: true }).click();
    await expect(page.getByText("dotagents sync --user")).toBeVisible();
    await expect(page.getByText("done")).toBeVisible();
  });
});
