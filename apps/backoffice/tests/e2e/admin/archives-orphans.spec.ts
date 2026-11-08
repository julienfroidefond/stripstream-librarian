import { test, expect, type Locator, type Page } from "@playwright/test";

import { uniqueName } from "../helpers";
import { deleteSeries, listArchivedSeries, seriesExists } from "../support/api";
import { createMetadataSeries, removeMetadataSeries } from "../support/metadata";

/**
 * Fixtures déterministes pour les onglets Orphan series et Archives.
 *
 * Une série de métadonnées sans aucun livre est bien un orphelin. Elle est
 * créée par l'API, retrouvée dans l'UI (en parcourant la pagination, car elle
 * peut tomber sur n'importe quelle page), puis archivée via l'API. L'UI doit
 * ensuite la retirer des orphelins et l'exposer dans les archives avec ses
 * auteurs et genres.
 *
 * La ligne d'archive n'a pas d'endpoint de suppression : elle est purgée par
 * le teardown global avec la bibliothèque `e2e-metadata`.
 */

async function waitForOrphansLoaded(page: Page): Promise<void> {
  await expect(
    page.locator("main").getByText(/No orphan series\.|\d+ orphan series/)
  ).toBeVisible();
}

/** Walk the orphan pages until the unique series is found (or pages end). */
async function findOrphanRow(page: Page, name: string): Promise<Locator | null> {
  const table = page.locator("main table").first();
  if ((await table.count()) === 0) return null;

  const row = table.getByRole("row").filter({ hasText: name });
  const next = page.getByRole("button", { name: "Next", exact: true });
  const pageInfo = page.getByText(/^Page \d+ of \d+$/);

  for (let guard = 0; guard < 100; guard++) {
    if ((await row.count()) > 0) return row.first();
    if ((await next.count()) === 0 || (await next.isDisabled())) return null;

    const before = await pageInfo.textContent();
    await next.click();
    await expect(pageInfo).not.toHaveText(before ?? "");
  }

  return null;
}

test.describe("paramètres — orphelins et archives", () => {
  test("archive une série orpheline et la retrouve dans les archives", async ({ page }) => {
    const author = uniqueName("E2E Orphan Author");
    const genre = uniqueName("E2E Orphan Genre");
    const series = await createMetadataSeries({ authors: [author], genres: [genre] });

    try {
      await page.goto("/settings?tab=orphans");
      await waitForOrphansLoaded(page);

      const orphanRow = await findOrphanRow(page, series.name);
      expect(orphanRow, "la série orpheline doit apparaître dans l'onglet").not.toBeNull();
      await expect(orphanRow!).toBeVisible();

      // Archivage direct par l'API : la suppression d'une série archive toujours.
      await deleteSeries(series.seriesId);

      await expect
        .poll(async () => {
          const archived = await listArchivedSeries();
          return archived.some((item) => item.name === series.name);
        })
        .toBe(true);

      // L'orphelin a disparu après rechargement.
      await page.reload();
      await waitForOrphansLoaded(page);
      const remaining = await findOrphanRow(page, series.name);
      expect(remaining, "l'orphelin archivé ne doit plus être listé").toBeNull();

      // L'archive expose les métadonnées de la série.
      await page.goto("/settings?tab=archives");
      const archiveRow = page
        .locator("main table")
        .first()
        .getByRole("row")
        .filter({ hasText: series.name });
      await expect(archiveRow).toBeVisible();
      await expect(archiveRow).toContainText(author);
      await expect(archiveRow).toContainText(genre);

      // Détail dépliable : cette fixture orpheline n'a aucun tome archivé.
      await archiveRow.click();
      await expect(page.getByText("No archived volume.", { exact: true })).toBeVisible();
    } finally {
      await removeMetadataSeries(series);
    }
  });

  test("annule puis confirme la suppression d'un orphelin", async ({ page }) => {
    const series = await createMetadataSeries();

    try {
      await page.goto("/settings?tab=orphans");
      await waitForOrphansLoaded(page);

      const row = await findOrphanRow(page, series.name);
      expect(row, "la série orpheline doit apparaître avant suppression").not.toBeNull();

      // Cancel must leave both the row and the database untouched.
      await row!.getByRole("button", { name: "Delete", exact: true }).click();
      const modal = page.getByTestId("modal-panel");
      await expect(modal.getByText("Delete this series?", { exact: true })).toBeVisible();
      await expect(modal).toContainText(series.name);
      await modal.getByRole("button", { name: "Cancel", exact: true }).click();
      await expect(modal).toBeHidden();
      await expect(row!).toBeVisible();
      expect(await seriesExists(series.seriesId)).toBe(true);

      // Confirming deletes this disposable fixture only.
      await row!.getByRole("button", { name: "Delete", exact: true }).click();
      await expect(modal.getByText("Delete this series?", { exact: true })).toBeVisible();
      await modal.getByRole("button", { name: "Delete", exact: true }).click();

      await expect
        .poll(async () => seriesExists(series.seriesId), { timeout: 10_000 })
        .toBe(false);
      await expect(page.getByText("1 series deleted", { exact: true })).toBeVisible();
      await expect(page.getByRole("row").filter({ hasText: series.name })).toHaveCount(0);
    } finally {
      await removeMetadataSeries(series);
    }
  });

  test("annule la suppression en masse des orphelins", async ({ page }) => {
    const series = await createMetadataSeries();

    try {
      await page.goto("/settings?tab=orphans");
      await waitForOrphansLoaded(page);

      const bulkButton = page.getByRole("button", {
        name: "Delete this page's selection",
        exact: true,
      });
      await expect(bulkButton).toBeEnabled();
      await bulkButton.click();

      const modal = page.getByTestId("modal-panel");
      await expect(
        modal.getByText("Delete the orphan series on this page?", { exact: true })
      ).toBeVisible();
      await expect(modal).toContainText("will be archived then deleted");

      let deleteRequests = 0;
      const countDelete = (request: { method(): string; url(): string }) => {
        if (request.method() === "DELETE" && request.url().endsWith("/api/series/orphans")) {
          deleteRequests += 1;
        }
      };
      page.on("request", countDelete);
      await modal.getByRole("button", { name: "Cancel", exact: true }).click();
      page.off("request", countDelete);

      await expect(modal).toBeHidden();
      expect(deleteRequests).toBe(0);
      expect(await seriesExists(series.seriesId)).toBe(true);
    } finally {
      await removeMetadataSeries(series);
    }
  });
});
