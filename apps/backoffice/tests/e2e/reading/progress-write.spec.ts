import { test, expect } from "@playwright/test";

import { openSeriesDetail } from "../helpers";
import {
  findUserByUsername,
  getBookReadingProgress,
  updateBookReadingProgress,
} from "../support/api";
import {
  createDisposableBook,
  createDisposableSeries,
  removeDisposableBook,
  removeDisposableSeries,
} from "../support/books";
import { BOOK_LIBRARY_NAME } from "../support/metadata";

/**
 * Écritures de progression sur un livre jetable, avec le lecteur `e2e-reader`
 * préparé par le setup. Le livre est créé dans `e2e-books` et supprimé dans
 * `finally`, donc aucune progression partagée n'est laissée derrière.
 */

const READER_USERNAME = "e2e-reader";

test.describe("progression de lecture — écritures", () => {
  test("marque lu puis non lu et persiste après rechargement", async ({ page }) => {
    test.setTimeout(120_000);

    const reader = await findUserByUsername(READER_USERNAME);
    if (!reader) {
      test.skip(true, `lecteur ${READER_USERNAME} introuvable`);
      return;
    }

    const book = await createDisposableBook();

    try {
      await page.goto(`/books/${book.id}`);

      const statusRow = page.locator("main h1 + div");
      await expect(statusRow.getByText("Unread", { exact: true })).toBeVisible();
      const markRead = page.getByRole("button", { name: "Mark as read", exact: true });
      await expect(markRead).toBeVisible();

      // Unread -> Read.
      await markRead.click();
      await expect(statusRow.getByText("Read", { exact: true })).toBeVisible();
      await expect(
        page.getByRole("button", { name: "Mark unread", exact: true })
      ).toBeVisible();
      await expect
        .poll(async () => (await getBookReadingProgress(book.id, reader.id)).status)
        .toBe("read");

      // Read -> Unread.
      await page.getByRole("button", { name: "Mark unread", exact: true }).click();
      await expect(statusRow.getByText("Unread", { exact: true })).toBeVisible();
      await expect(
        page.getByRole("button", { name: "Mark as read", exact: true })
      ).toBeVisible();
      await expect
        .poll(async () => (await getBookReadingProgress(book.id, reader.id)).status)
        .toBe("unread");

      // L'état est bien relu depuis l'API après un rechargement complet.
      await page.reload();
      await expect(
        page.locator("main h1 + div").getByText("Unread", { exact: true })
      ).toBeVisible();
    } finally {
      await removeDisposableBook(book);
    }
  });

  test("marque toute une série lue et conserve le statut de chaque tome", async ({ page }) => {
    test.setTimeout(120_000);

    const reader = await findUserByUsername(READER_USERNAME);
    if (!reader) {
      test.skip(true, `lecteur ${READER_USERNAME} introuvable`);
      return;
    }

    const series = await createDisposableSeries(2);

    try {
      for (const book of series.books) {
        await updateBookReadingProgress(book.id, reader.id, "unread");
      }

      await openSeriesDetail(page, {
        name: series.name,
        seriesId: series.seriesId,
        libraryName: BOOK_LIBRARY_NAME,
      });
      await expect(page.getByText("0/2 read", { exact: true })).toBeVisible();

      const [first, second] = series.books;
      const firstCard = page
        .locator("[data-cover-card]")
        .filter({ has: page.getByTitle(first.title) });
      await expect(firstCard).toHaveCount(1);
      await firstCard.getByRole("button", { name: "Read", exact: true }).click();
      await expect(firstCard.getByText("Read", { exact: true })).toBeVisible();
      await expect(page.getByText("1/2 read", { exact: true })).toBeVisible();
      await expect
        .poll(async () => (await getBookReadingProgress(first.id, reader.id)).status)
        .toBe("read");
      await expect
        .poll(async () => (await getBookReadingProgress(second.id, reader.id)).status)
        .toBe("unread");

      // L'action de série marque tous les tomes, puis la fiche affiche leur
      // statut individuel et le compteur global.
      await page.getByRole("button", { name: "Mark all read", exact: true }).click();
      await expect(page.getByText("2/2 read", { exact: true })).toBeVisible();
      await expect(page.getByRole("button", { name: "Mark unread", exact: true })).toBeVisible();

      for (const book of series.books) {
        const card = page
          .locator("[data-cover-card]")
          .filter({ has: page.getByTitle(book.title) });
        await expect(card.getByText("Read", { exact: true })).toBeVisible();
        await expect
          .poll(async () => (await getBookReadingProgress(book.id, reader.id)).status)
          .toBe("read");
      }

      await page.reload();
      await expect(page.getByText("2/2 read", { exact: true })).toBeVisible();
    } finally {
      await removeDisposableSeries(series);
    }
  });
});
