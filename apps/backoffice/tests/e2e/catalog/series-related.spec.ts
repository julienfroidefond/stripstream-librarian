import { test, expect } from "@playwright/test";

import { openSeriesDetail, uniqueName } from "../helpers";
import {
  findUserByUsername,
  updateBookReadingProgress,
  updateSeries,
} from "../support/api";
import { createDisposableSeries, removeDisposableSeries } from "../support/books";
import { BOOK_LIBRARY_NAME } from "../support/metadata";

/**
 * Séries liées et recommandations personnalisées.
 *
 * Deux séries jetables partagent un auteur unique au run. La première est
 * marquée lue par `e2e-reader`, ce qui doit produire :
 * - la seconde dans le carrousel « Similar Series » de la fiche source ;
 * - la seconde dans l'onglet Recommendations avec la raison « Shared author ».
 * Les deux séries sont supprimées dans `finally` (progression comprise).
 */

const READER_USERNAME = "e2e-reader";

test.describe("séries — liées et recommandations", () => {
  test("affiche une série liée puis la recommande après lecture", async ({ page }) => {
    test.setTimeout(90_000);

    const reader = await findUserByUsername(READER_USERNAME);
    if (!reader) {
      test.skip(true, `lecteur ${READER_USERNAME} introuvable`);
      return;
    }

    const source = await createDisposableSeries(1);
    const candidate = await createDisposableSeries(1);
    const author = uniqueName("E2E related author");

    try {
      for (const series of [source, candidate]) {
        await updateSeries(series.seriesId, {
          new_name: series.name,
          authors: [author],
          genres: [],
        });
      }

      await updateBookReadingProgress(source.books[0].id, reader.id, "read");

      // Fiche de la série lue : le carrousel ne doit proposer que la série
      // partageant l'auteur unique.
      await openSeriesDetail(page, {
        name: source.name,
        seriesId: source.seriesId,
        libraryName: BOOK_LIBRARY_NAME,
      });
      await expect(page.getByText("Similar Series", { exact: true })).toBeVisible();

      const relatedLink = page.locator(`a[href="/series/${candidate.seriesId}"]`);
      await expect(relatedLink).toBeVisible();
      await expect(relatedLink).toContainText(candidate.name);
      await expect(relatedLink.getByText("Same author", { exact: true })).toBeVisible();

      // Onglet personnalisé : la candidate est recommandée à cause de la lecture.
      await page.goto("/series?view=recommendations");
      await expect(
        page.getByRole("heading", { name: "Recommendations", exact: true })
      ).toBeVisible();

      const recommendation = page.getByRole("heading", {
        name: candidate.name,
        exact: true,
      });
      await expect(recommendation).toBeVisible();

      const card = recommendation.locator("xpath=ancestor::div[contains(@class,'p-4')][1]");
      await expect(card).toContainText("Because you read:");
      await expect(card).toContainText(source.name);
      await expect(card.getByText("Shared author", { exact: true })).toBeVisible();
    } finally {
      await removeDisposableSeries(candidate);
      await removeDisposableSeries(source);
    }
  });
});
