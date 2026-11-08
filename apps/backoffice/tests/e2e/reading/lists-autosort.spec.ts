import { test, expect } from "@playwright/test";

import { openSeriesDetail } from "../helpers";
import { createReadingList, deleteReadingList } from "../support/api";
import { createDisposableSeries, removeDisposableSeries } from "../support/books";
import { BOOK_LIBRARY_NAME } from "../support/metadata";

/**
 * Une série entièrement lue est renvoyée en fin de liste de lecture.
 *
 * Les positions stockées ne changent pas : l'API réordonne à la lecture pour
 * l'utilisateur actif. Le test marque une série lue via la fiche, recharge la
 * liste et vérifie l'ordre affiché.
 */

test.describe("listes de lecture — tri automatique", () => {
  test("déplace une série entièrement lue en fin de liste", async ({ page }) => {
    test.setTimeout(120_000);

    const finished = await createDisposableSeries(1);
    const unfinished = await createDisposableSeries(1);
    const list = await createReadingList(`e2e-autosort-${Date.now().toString(36)}`);
    const orderedNames = page.locator('main a[href^="/series/"] p.font-semibold');

    try {
      await page.goto("/reading-lists");
      const status = await page.evaluate(
        async ({ id, first, second }) => {
          const add = (seriesId: string) =>
            fetch(`/api/reading-lists/${id}/series`, {
              method: "POST",
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify({ series_id: seriesId }),
            });
          const firstResponse = await add(first);
          const secondResponse = await add(second);
          return [firstResponse.status, secondResponse.status];
        },
        { id: list.id, first: finished.seriesId, second: unfinished.seriesId }
      );
      expect(status).toEqual([204, 204]);

      await page.goto(`/reading-lists/${list.id}`);
      await expect(orderedNames).toHaveText([finished.name, unfinished.name]);

      await openSeriesDetail(page, {
        name: finished.name,
        seriesId: finished.seriesId,
        libraryName: BOOK_LIBRARY_NAME,
      });
      await page.getByRole("button", { name: "Mark all read", exact: true }).click();

      await page.goto(`/reading-lists/${list.id}`);
      await expect(orderedNames).toHaveText([unfinished.name, finished.name]);
    } finally {
      await deleteReadingList(list.id).catch(() => undefined);
      await removeDisposableSeries(finished);
      await removeDisposableSeries(unfinished);
    }
  });
});
