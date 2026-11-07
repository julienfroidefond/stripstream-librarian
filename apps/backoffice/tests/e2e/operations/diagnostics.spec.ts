import { test, expect } from "@playwright/test";

test.describe("diagnostics — santé de la stack", () => {
  test("affiche les indicateurs de la base, des services et du disque", async ({ page }) => {
    await page.goto("/diagnostics");

    await expect(page.locator("main h1")).toContainText("Diagnostics / Health");

    for (const label of ["API version", "Uptime", "Jobs in progress"]) {
      await expect(page.locator("main").getByText(label, { exact: true }).first()).toBeVisible();
    }

    for (const section of ["Services", "Database", "Disk space", "Indexing jobs"]) {
      await expect(page.locator("main").getByText(section, { exact: true }).first()).toBeVisible();
    }

    await expect(page.getByText("Applied migrations")).toBeVisible();
  });

  test("le bouton Refresh recharge le rapport", async ({ page }) => {
    await page.goto("/diagnostics");

    await page.getByRole("button", { name: "Refresh" }).click();

    await expect(page.locator("main h1")).toContainText("Diagnostics / Health");
  });
});
