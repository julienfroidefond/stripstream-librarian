import { test, expect } from "@playwright/test";

test.describe("progression de lecture", () => {
  test("affiche l'état de lecture de l'utilisateur actif", async ({ page }) => {
    await page.goto("/progress");

    await expect(page.locator("main h1")).toContainText("Progress");

    // Data-dependent: the e2e reader may not have read anything yet. The page
    // must still render either the empty state or the reading statistics.
    await expect(page.locator("main")).toContainText(
      /No reading activity|books read|series in progress/
    );
  });
});
