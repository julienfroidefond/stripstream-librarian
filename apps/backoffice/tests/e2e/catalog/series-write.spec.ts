import { test, expect } from "@playwright/test";

import { uniqueName } from "../helpers";
import {
  apiRequest,
  getSeriesMetadata,
  listArchivedSeries,
  updateSeries,
} from "../support/api";
import {
  createMetadataSeries,
  openMetadataSeries,
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
      await openMetadataSeries(page, series);

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

  test("édite les champs et verrouille la description contre les synchronisations", async ({
    page,
  }) => {
    test.setTimeout(60_000);

    const series = await createMetadataSeries();
    const description = uniqueName("E2E synopsis");
    const startYear = 2024;

    try {
      await openMetadataSeries(page, series);

      await page.getByRole("button", { name: /Edit series/ }).click();
      const editPanel = page.getByTestId("modal-panel");
      await expect(editPanel).toBeVisible();

      // Renseigne un champ simple et la description, puis verrouille la
      // description avec le cadenas situé à côté de son libellé.
      await editPanel.getByPlaceholder("e.g. 1990").fill(String(startYear));
      await editPanel.locator("textarea").fill(description);

      const descriptionField = editPanel.locator("textarea").locator("xpath=..");
      await descriptionField.getByTitle("Click to lock").click();
      await expect(
        descriptionField.getByTitle("Locked field (protected from syncs)")
      ).toBeVisible();
      await expect(editPanel.getByText("Locked fields will not be overwritten")).toBeVisible();

      await editPanel.getByRole("button", { name: "Save", exact: true }).click();
      await expect(editPanel).toBeHidden();

      await expect
        .poll(async () => {
          const metadata = await getSeriesMetadata(series.seriesId);
          return {
            description: metadata.description,
            startYear: metadata.start_year,
            locked: metadata.locked_fields.description === true,
          };
        })
        .toEqual({ description, startYear, locked: true });
    } finally {
      await removeMetadataSeries(series);
    }
  });

  test("déverrouille un champ protégé depuis la modale", async ({ page }) => {
    test.setTimeout(60_000);

    const series = await createMetadataSeries();
    const description = uniqueName("E2E locked synopsis");
    const updatedDescription = uniqueName("E2E unlocked synopsis");

    // Fixture : la description est verrouillée par l'API avant d'ouvrir la
    // fiche, pour que la modale rende un champ réellement protégé au premier
    // chargement (le fetch des métadonnées est mis en cache 60 s par Next).
    await updateSeries(series.seriesId, {
      new_name: series.name,
      authors: [],
      genres: [],
      description,
      start_year: 2023,
      locked_fields: { description: true },
    });

    try {
      await openMetadataSeries(page, series);

      await page.getByRole("button", { name: /Edit series/ }).click();
      const editPanel = page.getByTestId("modal-panel");
      await expect(editPanel).toBeVisible();

      const textarea = editPanel.locator("textarea");
      await expect(textarea).toHaveValue(description);

      const descriptionField = textarea.locator("xpath=..");
      await expect(
        descriptionField.getByTitle("Locked field (protected from syncs)")
      ).toBeVisible();
      await descriptionField.getByTitle("Locked field (protected from syncs)").click();
      await expect(descriptionField.getByTitle("Click to lock")).toBeVisible();

      // Le champ déverrouillé redevient éditable ; sa nouvelle valeur est
      // enregistrée avec le cadenas désactivé.
      await textarea.fill(updatedDescription);
      await editPanel.getByRole("button", { name: "Save", exact: true }).click();
      await expect(editPanel).toBeHidden();

      await expect
        .poll(async () => {
          const metadata = await getSeriesMetadata(series.seriesId);
          return {
            description: metadata.description,
            startYear: metadata.start_year,
            locked: metadata.locked_fields.description === true,
          };
        })
        .toEqual({ description: updatedDescription, startYear: 2023, locked: false });
    } finally {
      await removeMetadataSeries(series);
    }
  });
});
