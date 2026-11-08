import { test, expect } from "@playwright/test";

import { openSeriesDetail } from "../helpers";
import {
  findUserByUsername,
  updateBookReadingProgress,
} from "../support/api";
import { createDisposableSeries, removeDisposableSeries } from "../support/books";
import { BOOK_LIBRARY_NAME } from "../support/metadata";

/**
 * Vue admin de la progression multi-utilisateurs sur une fiche série.
 *
 * Le cookie `as_user_id` est retiré pour ce test : la fiche affiche alors la
 * progression de chaque lecteur au lieu des actions du lecteur actif. La série
 * jetable et la progression associée sont supprimées dans `finally`.
 */

const READER_USERNAME = "e2e-reader";

test.describe("progression de lecture — vue admin", () => {
  test("affiche la progression de chaque lecteur sur la fiche série", async ({
    page,
    context,
  }) => {
    test.setTimeout(90_000);

    const reader = await findUserByUsername(READER_USERNAME);
    if (!reader) {
      test.skip(true, `lecteur ${READER_USERNAME} introuvable`);
      return;
    }

    const series = await createDisposableSeries(2);

    try {
      const [first] = series.books;
      await updateBookReadingProgress(first.id, reader.id, "read");

      // Mode admin : aucun lecteur sélectionné dans le backoffice.
      await context.clearCookies({ name: "as_user_id" });

      await openSeriesDetail(page, {
        name: series.name,
        seriesId: series.seriesId,
        libraryName: BOOK_LIBRARY_NAME,
      });

      const label = page.getByText("Reader progress", { exact: true });
      await expect(label).toBeVisible();
      await expect(
        page.getByRole("button", { name: "Mark all read", exact: true })
      ).toHaveCount(0);

      const chip = label
        .locator("..")
        .getByText(reader.username, { exact: true })
        .locator("..");
      await expect(chip).toContainText(reader.username);
      await expect(chip.getByText("1", { exact: true })).toBeVisible();
      await expect(chip.getByText("/2", { exact: true })).toBeVisible();
    } finally {
      await removeDisposableSeries(series);
    }
  });
});
