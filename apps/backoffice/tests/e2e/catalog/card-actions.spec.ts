import { test, expect, type Page } from "@playwright/test";

import { uniqueName, waitForHydration } from "../helpers";
import {
  findUserByUsername,
  getBookReadingProgress,
  updateSeries,
} from "../support/api";
import { createDisposableSeries, removeDisposableSeries } from "../support/books";
import { createMetadataSeries, openMetadataSeries, removeMetadataSeries } from "../support/metadata";

const READER_USERNAME = "e2e-reader";

/** Ouvre la carte d'une série dans la liste filtrée. */
async function openSeriesCard(page: Page, seriesName: string): Promise<void> {
  await page.goto(`/series?q=${encodeURIComponent(seriesName)}`);
  await expect(page.locator("main h1")).toContainText("Series");
  await waitForHydration(page);
  await expect(page.locator(`[data-cover-card]:has(h3[title="${seriesName}"])`)).toBeVisible();
}

test.describe("actions de carte — lecture et rafraîchissement", () => {
  test("marque toute la série lue puis non lue et lance le refresh ciblé", async ({ page }) => {
    test.setTimeout(120_000);

    const reader = await findUserByUsername(READER_USERNAME);
    if (!reader) {
      test.skip(true, `lecteur ${READER_USERNAME} introuvable`);
      return;
    }

    const series = await createDisposableSeries(2);
    try {
      await openSeriesCard(page, series.name);
      const card = page.locator(`[data-cover-card]:has(h3[title="${series.name}"])`);

      await expect(card.getByText("0/2 read", { exact: true })).toBeVisible();
      await card.getByRole("button", { name: "Read", exact: true }).click();
      await expect(card.getByText("2/2 read", { exact: true })).toBeVisible();
      await expect(card.getByRole("button", { name: "Unread", exact: true })).toBeVisible();
      for (const book of series.books) {
        await expect
          .poll(async () => (await getBookReadingProgress(book.id, reader.id)).status)
          .toBe("read");
      }

      await card.getByRole("button", { name: "Unread", exact: true }).click();
      await expect(card.getByText("0/2 read", { exact: true })).toBeVisible();
      await expect(card.getByRole("button", { name: "Read", exact: true })).toBeVisible();
      for (const book of series.books) {
        await expect
          .poll(async () => (await getBookReadingProgress(book.id, reader.id)).status)
          .toBe("unread");
      }

      // Le refresh ciblé déclenche une Server Action Next (invalidation du tag).
      await card.getByRole("button", { name: "More actions" }).click();
      const [refreshResponse] = await Promise.all([
        page.waitForResponse(
          (response) =>
            response.request().method() === "POST" &&
            Boolean(response.request().headers()["next-action"])
        ),
        card.getByRole("menuitem", { name: "Refresh", exact: true }).click(),
      ]);
      expect(refreshResponse.ok()).toBe(true);
    } finally {
      await removeDisposableSeries(series);
    }
  });

  test("une modification API devient visible après le refresh ciblé de la fiche", async ({
    page,
  }) => {
    test.setTimeout(90_000);

    const series = await createMetadataSeries();
    const firstDescription = uniqueName("E2E refresh description before");
    const secondDescription = uniqueName("E2E refresh description after");

    try {
      await updateSeries(series.seriesId, {
        new_name: series.name,
        description: firstDescription,
      });
      await openMetadataSeries(page, series);
      await expect(page.locator("main")).toContainText(firstDescription);

      await updateSeries(series.seriesId, {
        new_name: series.name,
        description: secondDescription,
      });

      await page.getByRole("main").getByRole("button", { name: "Refresh", exact: true }).click();
      await expect(page.locator("main")).toContainText(secondDescription, { timeout: 10_000 });
    } finally {
      await removeMetadataSeries(series);
    }
  });
});
