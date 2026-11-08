import fs from "node:fs";
import path from "node:path";

import { test, expect } from "@playwright/test";

import { escapeRegExp, openSeriesDetail, uniqueName } from "../helpers";
import { createDisposableSeries, getBook, removeDisposableSeries } from "../support/books";
import { BOOK_LIBRARY_NAME } from "../support/metadata";

/**
 * Renommage physique des fichiers d'une série jetable.
 *
 * L'aperçu est recalculé avec un template local au run, puis les colonnes
 * Type/Volume sont modifiées (hors-série, tome 7) avant de revenir aux valeurs
 * d'origine. L'exécution passe par l'UI et les chemins renvoyés par l'API sont
 * vérifiés sur le système de fichiers ; la série et son dossier sont supprimés
 * dans `finally`.
 */

test.describe("séries — renommage des fichiers", () => {
  test("prévisualise les templates, corrige type/tome puis renomme les fichiers", async ({
    page,
  }) => {
    test.setTimeout(90_000);

    const series = await createDisposableSeries(2);
    const renamedStem = uniqueName("E2E renamed");

    try {
      const oldNames = (await Promise.all(series.books.map((book) => getBook(book.id))))
        .map((book) => (book.file_path ? path.basename(book.file_path) : null))
        .filter((name): name is string => name !== null)
        .sort();
      expect(oldNames).toHaveLength(2);

      await openSeriesDetail(page, {
        name: series.name,
        seriesId: series.seriesId,
        libraryName: BOOK_LIBRARY_NAME,
      });

      await page.getByRole("button", { name: "More", exact: true }).first().click();
      await page.getByRole("menuitem", { name: /Rename books/ }).click();

      const panel = page.getByTestId("modal-panel");
      await expect(panel).toBeVisible();
      await expect(
        panel.getByRole("heading", { name: "Rename books in series" })
      ).toBeVisible();
      await expect(panel.locator("tbody tr")).toHaveCount(2);

      const rowFor = (oldName: string) =>
        panel.locator("tbody tr").filter({ hasText: oldName });
      const firstRow = rowFor(oldNames[0]);
      const secondRow = rowFor(oldNames[1]);
      await expect(firstRow).toHaveCount(1);
      await expect(secondRow).toHaveCount(1);

      // Template propre au run : le nom de série reste lisible dans le chemin.
      await panel.getByRole("button", { name: /Naming templates/ }).click();
      const templateInput = panel
        .getByText("Filename template", { exact: true })
        .locator("..")
        .locator("input");
      await templateInput.fill(`${series.name} - ${renamedStem} {volume_padded}`);
      await panel.getByRole("button", { name: "Refresh preview", exact: true }).click();

      const newNameCell = (row: typeof firstRow) => row.locator("td").nth(1);
      await expect(newNameCell(firstRow)).toContainText(`${renamedStem} 1.cbz`);
      await expect(newNameCell(secondRow)).toContainText(`${renamedStem} 2.cbz`);

      // Correction ponctuelle du type : l'aperçu bascule sur le template HS.
      await firstRow.getByRole("combobox").selectOption("hs");
      await expect(newNameCell(firstRow)).toContainText(/HS\s+1/);

      // Correction du tome : le numéro corrigé est utilisé dans le nom.
      const volumeInput = firstRow.getByRole("spinbutton");
      await volumeInput.fill("7");
      await volumeInput.blur();
      await expect(newNameCell(firstRow)).toContainText(/HS\s+7/);

      // Retour aux valeurs stockées avant l'exécution réelle.
      await firstRow.getByRole("combobox").selectOption("regular");
      await volumeInput.fill("1");
      await volumeInput.blur();
      await expect(newNameCell(firstRow)).toContainText(`${renamedStem} 1.cbz`);

      const expectedNames = [
        (await newNameCell(firstRow).innerText()).trim(),
        (await newNameCell(secondRow).innerText()).trim(),
      ];
      expect(expectedNames[0]).toMatch(
        new RegExp(`${escapeRegExp(renamedStem)}\\s+1\\.cbz$`)
      );
      expect(expectedNames[1]).toMatch(
        new RegExp(`${escapeRegExp(renamedStem)}\\s+2\\.cbz$`)
      );

      const [executeResponse] = await Promise.all([
        page.waitForResponse((response) => {
          if (response.request().method() !== "POST") return false;
          if (new URL(response.url()).pathname !== `/api/series/${series.seriesId}/rename-books`) {
            return false;
          }
          const body = response.request().postDataJSON() as { mode?: string };
          return body.mode === "execute";
        }),
        panel.getByRole("button", { name: "Apply renames", exact: true }).click(),
      ]);
      expect(executeResponse.ok()).toBe(true);
      await expect(
        panel.getByText("2 book(s) renamed successfully", { exact: true })
      ).toBeVisible();
      await expect(panel.getByText("Failed to rename some books")).toHaveCount(0);

      await expect
        .poll(async () => {
          const books = await Promise.all(series.books.map((book) => getBook(book.id)));
          return books
            .map((book) => (book.file_path ? path.basename(book.file_path) : null))
            .sort();
        })
        .toEqual([...expectedNames].sort());

      for (const name of expectedNames) {
        expect(fs.existsSync(path.join(series.directory, name))).toBe(true);
      }
    } finally {
      await removeDisposableSeries(series);
    }
  });
});
