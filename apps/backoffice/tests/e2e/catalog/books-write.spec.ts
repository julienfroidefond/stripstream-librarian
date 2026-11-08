import fs from "node:fs";

import { test, expect } from "@playwright/test";

import { uniqueName } from "../helpers";
import { getSeriesDetails, listArchivedSeries } from "../support/api";
import {
  createDisposableBook,
  getBook,
  getBookOrNull,
  removeDisposableBook,
} from "../support/books";

test.describe("livres — actions", () => {
  test("édite, télécharge puis supprime un livre", async ({ page }) => {
    test.setTimeout(120_000);

    const book = await createDisposableBook();
    const editedTitle = uniqueName("E2E edited book");
    if (!book.seriesId) throw new Error("Le livre jetable doit appartenir à une série");
    const series = await getSeriesDetails(book.seriesId);

    try {
      await page.goto(`/books/${book.id}`);
      await expect(page.locator("main h1")).toHaveText(book.title);

      // Edit metadata through the modal.
      await page.getByRole("button", { name: /Edit metadata/ }).click();
      const panel = page.getByTestId("modal-panel");
      await expect(panel).toBeVisible();
      await panel.getByPlaceholder("Book title").fill(editedTitle);
      await panel.getByRole("button", { name: "Save", exact: true }).click();
      await expect(panel).toBeHidden();
      await expect.poll(async () => (await getBook(book.id)).title).toBe(editedTitle);

      await page.reload();
      await expect(page.locator("main h1")).toHaveText(editedTitle);

      // Download the original file through the More menu.
      await page.getByRole("button", { name: "More", exact: true }).click();
      const downloadPromise = page.waitForEvent("download");
      await page.getByRole("menuitem", { name: "Download original file" }).click();
      const download = await downloadPromise;
      expect(download.suggestedFilename()).toMatch(/\.cbz$/i);
      await download.delete();

      // Cancel deletion first, then confirm it.
      await page.getByRole("button", { name: "More", exact: true }).click();
      await page.getByRole("menuitem", { name: /Delete/ }).click();
      await expect(panel).toBeVisible();
      await panel.getByRole("button", { name: "Cancel", exact: true }).click();
      await expect(panel).toBeHidden();
      expect(await getBookOrNull(book.id)).not.toBeNull();

      await page.getByRole("button", { name: "More", exact: true }).click();
      await page.getByRole("menuitem", { name: /Delete/ }).click();
      await expect(panel).toBeVisible();
      const deleteResponse = page.waitForResponse(
        (response) =>
          response.request().method() === "DELETE" &&
          new URL(response.url()).pathname === `/api/books/${book.id}`
      );
      await panel.getByRole("button", { name: "Delete", exact: true }).click();
      expect((await deleteResponse).status()).toBe(200);
      await expect.poll(async () => getBookOrNull(book.id)).toBeNull();
      expect(fs.existsSync(book.filePath)).toBe(false);

      // It was the only book of its series: the series is archived.
      await expect
        .poll(async () => {
          const archived = await listArchivedSeries();
          return archived.some((item) => item.name === series.name);
        })
        .toBe(true);
    } finally {
      await removeDisposableBook(book);
    }
  });

  test("verrouille puis déverrouille un champ du livre", async ({ page }) => {
    test.setTimeout(120_000);

    const book = await createDisposableBook();

    try {
      await page.goto(`/books/${book.id}`);
      await expect(page.locator("main h1")).toHaveText(book.title);

      await page.getByRole("button", { name: /Edit metadata/ }).click();
      const panel = page.getByTestId("modal-panel");
      await expect(panel).toBeVisible();

      const titleField = panel.getByPlaceholder("Book title").locator("xpath=..");
      await titleField.getByTitle("Click to lock this field").click();
      await expect(
        titleField.getByTitle("Locked field (protected from syncs)")
      ).toBeVisible();
      await expect(panel.getByText("Locked fields will not be overwritten")).toBeVisible();

      await panel.getByRole("button", { name: "Save", exact: true }).click();
      await expect(panel).toBeHidden();
      await expect
        .poll(async () => (await getBook(book.id)).locked_fields?.title === true)
        .toBe(true);

      // Le verrouillage est relu depuis l'API après un rechargement complet.
      await page.reload();
      await page.getByRole("button", { name: /Edit metadata/ }).click();
      await expect(panel).toBeVisible();
      const reloadedTitleField = panel.getByPlaceholder("Book title").locator("xpath=..");
      await expect(
        reloadedTitleField.getByTitle("Locked field (protected from syncs)")
      ).toBeVisible();

      await reloadedTitleField.getByTitle("Locked field (protected from syncs)").click();
      await expect(reloadedTitleField.getByTitle("Click to lock this field")).toBeVisible();
      await panel.getByRole("button", { name: "Save", exact: true }).click();
      await expect(panel).toBeHidden();
      await expect
        .poll(async () => (await getBook(book.id)).locked_fields?.title === true)
        .toBe(false);
    } finally {
      await removeDisposableBook(book);
    }
  });
});
