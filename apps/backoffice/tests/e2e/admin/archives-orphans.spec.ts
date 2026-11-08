import { test, expect, type Locator, type Page } from "@playwright/test";

import { uniqueName } from "../helpers";
import { deleteSeries, listArchivedSeries } from "../support/api";
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
    } finally {
      await removeMetadataSeries(series);
    }
  });
});
