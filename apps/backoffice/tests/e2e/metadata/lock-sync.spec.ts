import { test, expect } from "@playwright/test";

import { uniqueName } from "../helpers";
import {
  approveMetadataLink,
  createMetadataMatch,
  deleteMetadataLink,
  getSeriesMetadata,
  updateSeries,
} from "../support/api";
import { createMetadataSeries, removeMetadataSeries } from "../support/metadata";

/**
 * Verrouillage par champ pendant une synchronisation de métadonnées.
 *
 * La description est verrouillée avant l'ouverture de la modale ; la promotion
 * d'un second lien (via « Set as primary ») doit mettre à jour les auteurs et
 * genres mais conserver la description locale. `sync_books` est neutralisé
 * dans la requête UI pour ne jamais appeler un provider externe.
 */

test.describe("métadonnées — verrouillage et synchronisation", () => {
  test("préserve un champ verrouillé et met à jour les champs libres", async ({ page }) => {
    test.setTimeout(60_000);

    const lockedDescription = uniqueName("E2E locked description");
    const providerDescription = uniqueName("E2E provider description");
    const providerAuthor = uniqueName("E2E provider author");
    const providerGenre = uniqueName("E2E provider genre");
    const series = await createMetadataSeries();
    const linkIds: string[] = [];

    try {
      await updateSeries(series.seriesId, {
        new_name: series.name,
        authors: [],
        genres: [],
        description: lockedDescription,
        locked_fields: { description: true },
      });

      const first = await createMetadataMatch({
        library_id: series.libraryId,
        series_name: series.name,
        provider: "open_library",
        external_id: uniqueName("e2e-open-library-lock"),
        title: series.name,
        confidence: 0.92,
        metadata_json: { description: uniqueName("E2E primary description") },
      });
      linkIds.push(first.id);
      await approveMetadataLink(first.id);

      const second = await createMetadataMatch({
        library_id: series.libraryId,
        series_name: series.name,
        provider: "google_books",
        external_id: uniqueName("e2e-google-books-lock"),
        title: series.name,
        confidence: 0.88,
        metadata_json: {
          description: providerDescription,
          authors: [providerAuthor],
          genres: [providerGenre],
        },
      });
      linkIds.push(second.id);
      await approveMetadataLink(second.id);

      // Ouvre la fiche en réchauffant le cache des bibliothèques.
      const listUrl = `/series?q=${encodeURIComponent(series.name)}&e2e=${encodeURIComponent(
        uniqueName("lock-detail")
      )}`;
      await expect(async () => {
        await page.goto(listUrl);
        await expect(page.getByTitle(series.name)).toBeVisible();
        await expect(
          page.locator(`select[name="library"] option[value="${series.libraryId}"]`)
        ).toHaveCount(1, { timeout: 1500 });
      }).toPass({ timeout: 20_000 });

      await page.getByRole("link", { name: series.name, exact: true }).click();
      await expect(page).toHaveURL(new RegExp(`/series/${series.seriesId}$`));
      await expect(page.locator("main h1")).toHaveText(series.name);

      await page.getByRole("button", { name: "More", exact: true }).first().click();
      await page.getByRole("menuitem", { name: "Metadata" }).click();
      const panel = page.getByTestId("modal-panel");
      await expect(panel).toBeVisible();

      await page.route(/\/api\/metadata\/links(?:\?.*)?$/, async (route) => {
        if (route.request().method() !== "PATCH") {
          await route.continue();
          return;
        }
        const body = route.request().postDataJSON() as Record<string, unknown>;
        await route.continue({
          postData: JSON.stringify({ ...body, sync_books: false }),
        });
      });

      const [patchResponse] = await Promise.all([
        page.waitForResponse(
          (response) =>
            response.request().method() === "PATCH" &&
            new URL(response.url()).pathname === "/api/metadata/links"
        ),
        panel.getByRole("button", { name: "Set as primary", exact: true }).click(),
      ]);
      expect(patchResponse.ok()).toBe(true);

      await expect
        .poll(async () => (await getSeriesMetadata(series.seriesId)).description)
        .toBe(lockedDescription);
      await expect
        .poll(async () => (await getSeriesMetadata(series.seriesId)).authors)
        .toEqual([providerAuthor]);
      await expect
        .poll(async () => (await getSeriesMetadata(series.seriesId)).genres)
        .toEqual([providerGenre]);
      await expect
        .poll(async () => (await getSeriesMetadata(series.seriesId)).locked_fields.description)
        .toBe(true);

      await page.keyboard.press("Escape");
      await expect(panel).toBeHidden();
      await expect(page.locator("main")).toContainText(lockedDescription);
      await expect(page.locator("main").getByText(providerAuthor, { exact: true })).toBeVisible();
      await expect(page.locator("main")).not.toContainText(providerDescription);
    } finally {
      for (const id of linkIds) {
        await deleteMetadataLink(id).catch(() => undefined);
      }
      await removeMetadataSeries(series);
    }
  });
});
