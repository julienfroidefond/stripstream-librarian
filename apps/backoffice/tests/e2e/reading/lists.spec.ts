import { test, expect } from "@playwright/test";

import { apiRequest } from "../support/api";

/**
 * Reading lists are the only domain with a full create/read/update/delete UI.
 * The flow is kept in a single test because it mutates state sequentially;
 * `fullyParallel` only parallelizes tests, not steps.
 */

type SeriesPage = { items: Array<{ series_id: string; name: string }> };

test.describe("listes de lecture — cycle de vie", () => {
  test("crée une liste, ajoute une série, retire la série puis supprime la liste", async ({
    page,
  }) => {
    const seriesPage = await apiRequest<SeriesPage>("/series?limit=1&page=1");
    const series = seriesPage.items[0];
    test.skip(!series, "Aucune série en base");

    const name = `e2e-list-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`;
    page.on("dialog", (dialog) => dialog.accept());

    try {
      await page.goto("/reading-lists");
      await expect(page.locator("main h1")).toContainText("Reading Lists");

      // Create
      await page.getByRole("button", { name: "New List" }).first().click();
      const panel = page.getByTestId("modal-panel");
      await expect(panel).toBeVisible();
      await expect(panel).toContainText("Create Reading List");
      await panel.getByPlaceholder("List name...").fill(name);
      await panel.getByPlaceholder("Description...").fill("Créée par le test e2e");
      await panel.getByRole("button", { name: "New List" }).click();
      await expect(panel).toBeHidden();

      await expect(page.getByTitle(name)).toBeVisible();

      // Open the detail page
      await page.getByTitle(name).click();
      await page.waitForURL(/\/reading-lists\/[^/]+$/);
      await expect(page.locator("main h1")).toContainText(name);
      await expect(page.getByText("No series in this list.")).toBeVisible();

      // Add a series
      await page.getByRole("button", { name: "Add a series" }).click();
      await expect(panel).toBeVisible();
      await panel.getByPlaceholder("Search for a series...").fill(series!.name);
      const result = panel.getByRole("button").filter({ hasText: series!.name }).first();
      await expect(result).toBeVisible();
      await result.click();
      await expect(panel.getByText("Already in list")).toBeVisible();
      await panel.getByTestId("modal-close").click();
      await expect(panel).toBeHidden();

      await expect(page.locator("main").getByText(series!.name, { exact: true }).first()).toBeVisible();

      // Remove the series
      await page.locator('button[title="Remove from list"]').first().click({ force: true });
      await expect(page.getByText("No series in this list.")).toBeVisible();

      // Back to the list and delete it
      await page.locator('a[title="Back to lists"]').click();
      await page.waitForURL(/\/reading-lists$/);

      const card = page
        .getByTitle(name)
        .locator("xpath=ancestor::div[contains(@class,'rounded-lg')][1]");
      await card.getByRole("button", { name: "Delete" }).click();
      await expect(page.getByTitle(name)).toHaveCount(0);
    } finally {
      const lists = await apiRequest<Array<{ id: string; name: string }>>("/reading-lists").catch(
        () => []
      );
      await Promise.all(
        lists
          .filter((list) => list.name === name)
          .map((list) =>
            apiRequest(`/reading-lists/${list.id}`, { method: "DELETE" }).catch(() => undefined)
          )
      );
    }
  });
});
