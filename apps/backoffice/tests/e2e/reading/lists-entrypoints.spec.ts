import { test, expect } from "@playwright/test";

import { uniqueName, waitForHydration } from "../helpers";
import {
  createReadingList,
  deleteReadingList,
  getReadingList,
} from "../support/api";
import { createMetadataSeries, removeMetadataSeries } from "../support/metadata";

/**
 * Entrées « Ajouter à une liste de lecture » depuis une carte de série puis
 * depuis la fiche détail. Les listes sont créées par l'API, l'ajout passe par
 * l'UI, et la fiche de liste vérifie le badge « No books » d'une série sans
 * tome.
 */

test.describe("listes de lecture — entrées et badge série vide", () => {
  test("ajoute depuis la carte et la fiche, puis affiche le badge vide", async ({ page }) => {
    test.setTimeout(90_000);

    const cardList = await createReadingList(uniqueName("e2e-entry-card"));
    const detailList = await createReadingList(uniqueName("e2e-entry-detail"));
    const series = await createMetadataSeries();

    try {
      await page.goto(`/series?q=${encodeURIComponent(series.name)}`);
      await expect(page.locator("main h1")).toContainText("Series");
      await waitForHydration(page);

      const card = page.locator(`[data-cover-card]:has(h3[title="${series.name}"])`);
      await expect(card).toBeVisible();
      await card.getByRole("button", { name: "More actions" }).click();
      await card.getByRole("menuitem", { name: "Add to reading list" }).click();

      let panel = page.getByTestId("modal-panel");
      await expect(panel).toBeVisible();
      await expect(panel.getByText(cardList.name, { exact: true })).toBeVisible();
      await panel.getByRole("button").filter({ hasText: cardList.name }).click();
      await expect
        .poll(async () => (await getReadingList(cardList.id)).items.length)
        .toBe(1);
      await panel.getByTestId("modal-close").click();
      await expect(panel).toBeHidden();

      // Même parcours depuis la fiche détail, vers une seconde liste.
      await page.getByRole("link", { name: series.name, exact: true }).first().click();
      await expect(page).toHaveURL(new RegExp(`/series/${series.seriesId}$`));
      await expect(page.locator("main h1")).toHaveText(series.name);
      await page.getByRole("button", { name: "More", exact: true }).click();
      await page.getByRole("menuitem", { name: "Add to reading list" }).click();
      panel = page.getByTestId("modal-panel");
      await expect(panel).toBeVisible();
      await expect(panel.getByText(detailList.name, { exact: true })).toBeVisible();
      await panel.getByRole("button").filter({ hasText: detailList.name }).click();
      await expect
        .poll(async () => (await getReadingList(detailList.id)).items.length)
        .toBe(1);
      await panel.getByTestId("modal-close").click();
      await expect(panel).toBeHidden();

      // La fiche de liste matérialise la série sans tome par un badge.
      await page.goto(`/reading-lists/${detailList.id}`);
      await expect(page.locator("main h1")).toHaveText(detailList.name);
      const row = page
        .getByText(series.name, { exact: true })
        .locator("xpath=ancestor::div[contains(@class,'group')][1]");
      await expect(row).toBeVisible();
      await expect(row.getByText("No books", { exact: true })).toBeVisible();
    } finally {
      await deleteReadingList(cardList.id).catch(() => undefined);
      await deleteReadingList(detailList.id).catch(() => undefined);
      await removeMetadataSeries(series);
    }
  });
});
