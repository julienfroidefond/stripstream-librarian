import { test, expect } from "@playwright/test";

import { isDownloadsEnabled } from "../support/api";

test.describe("téléchargements — feature flag", () => {
  test("la page est accessible quand les téléchargements sont activés", async ({ page }) => {
    test.skip(!(await isDownloadsEnabled()), "downloads_enabled=false");

    await page.goto("/downloads");

    await expect(page.locator("main h1")).toContainText("Downloads");
    await expect(page.getByRole("button", { name: "Refresh" }).first()).toBeVisible();
    await expect(page.getByRole("button", { name: /In progress/ }).first()).toBeVisible();
  });

  test("redirige vers le dashboard quand les téléchargements sont désactivés", async ({
    page,
  }) => {
    test.skip(await isDownloadsEnabled(), "downloads_enabled=true");

    await page.goto("/downloads");

    await expect(page).toHaveURL(/\/$/);
    await expect(page.locator("main h1")).toContainText("Dashboard");
  });
});
