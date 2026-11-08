import { test, expect } from "@playwright/test";

import { uniqueName } from "../helpers";
import {
  addSeriesToReadingList,
  apiRequest,
  createReadingList,
  deleteReadingList,
  getReadingList,
} from "../support/api";
import { createMetadataSeries, removeMetadataSeries } from "../support/metadata";

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

  test("renomme, réordonne et regroupe une liste existante", async ({ page }) => {
    const first = await createMetadataSeries({ name: uniqueName("E2E liste A") });
    const second = await createMetadataSeries({ name: uniqueName("E2E liste B") });
    const originalName = uniqueName("e2e-list-manage");
    const renamed = uniqueName("e2e-list-renamed");
    let listId: string | undefined;

    try {
      const list = await createReadingList(originalName, "Liste gérée par le test e2e");
      listId = list.id;
      await addSeriesToReadingList(list.id, first.seriesId);
      await addSeriesToReadingList(list.id, second.seriesId);
      await expect
        .poll(async () => (await getReadingList(list.id)).items.length)
        .toBe(2);

      // A no-op PATCH through the Next route revalidates the 30 s reading-list cache.
      await page.goto("/reading-lists");
      const status = await page.evaluate(
        async ({ id, name }) => {
          const response = await fetch(`/api/reading-lists/${id}`, {
            method: "PATCH",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ name }),
          });
          return response.status;
        },
        { id: list.id, name: originalName }
      );
      expect(status).toBe(200);

      await page.goto(`/reading-lists/${list.id}`);
      await expect(page.locator("main h1")).toHaveText(originalName);
      await expect(page.locator("main").getByText(first.name, { exact: true })).toBeVisible();
      await expect(page.locator("main").getByText(second.name, { exact: true })).toBeVisible();

      // Rename through the inline editor.
      await page.getByTitle("Edit list").click();
      const nameInput = page.locator("main input[type=text]").first();
      await expect(nameInput).toHaveValue(originalName);
      await nameInput.fill(renamed);
      await page.locator("main").getByRole("button", { name: "Save", exact: true }).click();
      await expect(page.locator("main h1")).toHaveText(renamed);
      await expect
        .poll(async () => (await getReadingList(list.id)).name)
        .toBe(renamed);

      // Reorder: move the first series down, then verify the persisted order.
      const firstRow = page
        .locator("main")
        .getByText(first.name, { exact: true })
        .locator("xpath=ancestor::div[contains(@class,'group')][1]");
      await firstRow.locator('button[title="Move down"]').click({ force: true });
      await expect
        .poll(async () => (await getReadingList(list.id)).items.map((item) => item.id))
        .toEqual([second.seriesId, first.seriesId]);

      // Next finishes tag revalidation just after the mutation response, so poll
      // the grouped view instead of racing the following document request.
      await expect
        .poll(
          async () => {
            await page.goto("/series?group_by=reading_list");
            return page.getByTitle(renamed).count();
          },
          { timeout: 15_000 }
        )
        .toBeGreaterThan(0);
      await expect(page.getByTitle(renamed)).toBeVisible();
    } finally {
      if (listId) {
        await deleteReadingList(listId).catch(() => undefined);
      }
      await removeMetadataSeries(first);
      await removeMetadataSeries(second);
    }
  });
});
