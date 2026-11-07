import { test, expect } from "@playwright/test";

import { uniqueName } from "../helpers";
import { apiRequest, listArchivedSeries } from "../support/api";
import {
  createMetadataSeries,
  METADATA_LIBRARY_NAME,
  removeMetadataSeries,
} from "../support/metadata";

/**
 * Écritures sur une série de la bibliothèque temporaire : renommage via la
 * modale d'édition, suppression via le menu d'actions, puis vérification de
 * l'archive et de la disparition de la recherche.
 *
 * La série est créée par l'API (fixture), mais toutes les mutations passent
 * par l'UI. La ligne d'archive éventuellement laissée par le test est purgée
 * par le teardown global avec la bibliothèque `e2e-metadata`.
 */

test.describe("séries — édition et suppression", () => {
  test("renomme, supprime puis archive une série avec ses métadonnées", async ({ page }) => {
    // Le dev server compile la fiche série et les Paramètres à la demande ;
    // avec plusieurs workers, la première visite peut dépasser 30 s.
    test.setTimeout(60_000);

    const author = uniqueName("E2E Author");
    const genre = uniqueName("E2E Genre");
    const series = await createMetadataSeries({ authors: [author], genres: [genre] });
    const renamed = uniqueName("E2E renamed series");

    try {
      // La bibliothèque `e2e-metadata` est recréée à chaque run avec un
      // nouvel identifiant. La fiche série résout sa bibliothèque via
      // `fetchLibraries` (cache Next de 30 s) : on passe par la liste, qui
      // réchauffe ce cache, et on réessaie tant que l'option n'est pas là.
      const listUrl = `/series?q=${encodeURIComponent(series.name)}&e2e=${encodeURIComponent(
        uniqueName("detail")
      )}`;
      await expect(async () => {
        await page.goto(listUrl);
        await expect(page.getByTitle(series.name)).toBeVisible();
        await expect(
          page.locator('select[name="library"] option', { hasText: METADATA_LIBRARY_NAME })
        ).toHaveCount(1, { timeout: 1500 });
      }).toPass({ timeout: 20000 });

      await page.getByRole("link", { name: series.name, exact: true }).click();
      await expect(page).toHaveURL(new RegExp(`/series/${series.seriesId}$`));

      const heading = page.locator("main h1");
      await expect(heading).toHaveText(series.name);

      // Renommage via la modale d'édition.
      await page.getByRole("button", { name: /Edit series/ }).click();
      const editPanel = page.getByTestId("modal-panel");
      await expect(editPanel).toBeVisible();

      const nameInput = editPanel.getByPlaceholder("Series name");
      await expect(nameInput).toHaveValue(series.name);
      await nameInput.fill(renamed);
      await editPanel.getByRole("button", { name: "Save", exact: true }).click();
      await expect(editPanel).toBeHidden();

      // Le renommage est bien persisté côté API avant de vérifier l'affichage.
      await expect
        .poll(async () => {
          const details = await apiRequest<{ name: string }>(
            `/series/${series.seriesId}/details`
          );
          return details.name;
        })
        .toBe(renamed);

      // Le nouveau nom est bien persisté côté API après un rechargement.
      await page.reload();
      await expect(page.locator("main h1")).toHaveText(renamed);

      // Suppression via le menu d'actions puis confirmation.
      await page.getByRole("button", { name: "More", exact: true }).first().click();
      await page.getByRole("menuitem", { name: /Delete series/ }).click();

      const deletePanel = page.getByTestId("modal-panel");
      await expect(deletePanel).toBeVisible();
      await deletePanel.getByRole("button", { name: "Delete series", exact: true }).click();
      await expect(page).toHaveURL(/\/series(\?.*)?$/);

      // L'API a archivé la série (DELETE /series archive toujours).
      await expect
        .poll(async () => {
          const archived = await listArchivedSeries();
          return archived.some((item) => item.name === renamed);
        })
        .toBe(true);

      // La recherche ne renvoie plus la série supprimée.
      await page.goto(`/series?q=${encodeURIComponent(renamed)}`);
      await expect(page.locator("main")).toContainText("No series found matching your filters");
      await expect(page.getByTitle(renamed)).toHaveCount(0);

      // L'onglet Archives expose la ligne avec ses auteurs et genres.
      await page.goto("/settings?tab=archives");
      const row = page
        .locator("main table")
        .first()
        .getByRole("row")
        .filter({ hasText: renamed });
      await expect(row).toBeVisible();
      await expect(row).toContainText(author);
      await expect(row).toContainText(genre);
    } finally {
      await removeMetadataSeries(series);
    }
  });
});
