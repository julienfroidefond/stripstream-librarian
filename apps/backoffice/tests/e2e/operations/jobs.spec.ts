import { test, expect } from "@playwright/test";

test.describe("jobs d'indexation", () => {
  test("la liste et les filtres sont affichés", async ({ page }) => {
    await page.goto("/jobs");

    await expect(page.locator("main h1")).toContainText("Indexing jobs");
    await expect(page.getByRole("button", { name: /All actions/ })).toBeVisible();
    const firstSelect = page.locator("main select").first();
    await expect(firstSelect).toBeVisible();
    await expect(firstSelect).toContainText("All types");
  });

  test("ouvre le détail d'un job", async ({ page }) => {
    await page.goto("/jobs");

    const firstJob = page.locator('a[href^="/jobs/"]').first();
    test.skip((await firstJob.count()) === 0, "Aucun job en base");

    await firstJob.click();
    await page.waitForURL(/\/jobs\/[^/]+$/);

    await expect(page.getByRole("heading", { name: "Job details" })).toBeVisible();
    await expect(page.getByRole("link", { name: "Back to jobs" })).toBeVisible();
  });

  test("la palette d'actions filtre et se ferme avec Échap", async ({ page }) => {
    await page.goto("/jobs");

    await page.getByRole("button", { name: /All actions/ }).click();

    const dialog = page.getByRole("dialog", { name: "All actions" });
    await expect(dialog).toBeVisible();

    const search = dialog.getByPlaceholder("Search for an action…");
    await expect(search).toBeFocused();
    await search.fill("zzz-e2e-no-action");
    await expect(dialog.getByText("No action matches your search.")).toBeVisible();

    await search.fill("");
    await expect(dialog.getByText("Indexation", { exact: true }).first()).toBeVisible();

    await page.keyboard.press("Escape");
    await expect(dialog).toBeHidden();
  });
});
