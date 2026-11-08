import { test, expect } from "@playwright/test";

import { uniqueName } from "../helpers";
import {
  approveMetadataLink,
  createMetadataMatch,
  deleteMetadataLink,
  getSeriesMetadata,
} from "../support/api";
import { createMetadataSeries, removeMetadataSeries } from "../support/metadata";

/**
 * Synchronisation des métadonnées d'une série depuis la modale « Metadata ».
 *
 * Deux liens approuvés sont créés par l'API avec des `metadata_json`
 * déterministes ; la promotion du lien secondaire passe par l'UI et déclenche
 * `PATCH /metadata/links/:id` avec `sync_series: true`. Le backend fusionne les
 * JSON déjà stockés ; l'interception réseau ci-dessous neutralise `sync_books`
 * pour éviter tout appel à un provider externe.
 */

test.describe("métadonnées — synchronisation", () => {
  test("promeut un lien secondaire et synchronise les champs de la série", async ({ page }) => {
    // Le dev server compile la fiche série et la modale à la demande.
    test.setTimeout(60_000);

    const author = uniqueName("E2E synced author");
    const genre = uniqueName("E2E synced genre");
    const description = uniqueName("E2E synced description");
    const series = await createMetadataSeries();
    const linkIds: string[] = [];

    try {
      const first = await createMetadataMatch({
        library_id: series.libraryId,
        series_name: series.name,
        provider: "open_library",
        external_id: uniqueName("e2e-open-library"),
        title: series.name,
        confidence: 0.91,
        metadata_json: {
          description: uniqueName("E2E primary description"),
          authors: [uniqueName("E2E primary author")],
          genres: [uniqueName("E2E primary genre")],
        },
      });
      linkIds.push(first.id);
      await approveMetadataLink(first.id);

      const second = await createMetadataMatch({
        library_id: series.libraryId,
        series_name: series.name,
        provider: "google_books",
        external_id: uniqueName("e2e-google-books"),
        title: series.name,
        confidence: 0.87,
        metadata_json: { description, authors: [author], genres: [genre] },
      });
      linkIds.push(second.id);
      await approveMetadataLink(second.id);

      // La bibliothèque `e2e-metadata` est recréée à chaque run avec un nouvel
      // identifiant. La fiche série résout sa bibliothèque via `fetchLibraries`
      // (cache Next de 30 s) : on réchauffe la liste et on réessaie.
      const listUrl = `/series?q=${encodeURIComponent(series.name)}&e2e=${encodeURIComponent(
        uniqueName("detail")
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
      await expect(panel.getByText("Linked providers", { exact: true })).toBeVisible();

      // L'UI demande aussi `sync_books: true`, ce qui interroge le provider
      // externe pour récupérer les tomes. Le test ne dépend pas du réseau : on
      // vérifie le corps envoyé par l'UI puis on désactive uniquement cette
      // partie avant de laisser la requête atteindre le route handler.
      let uiPatchBody: Record<string, unknown> | null = null;
      await page.route(/\/api\/metadata\/links(?:\?.*)?$/, async (route) => {
        if (route.request().method() !== "PATCH") {
          await route.continue();
          return;
        }
        const body = route.request().postDataJSON() as Record<string, unknown>;
        uiPatchBody = body;
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
      expect(uiPatchBody).toMatchObject({
        is_primary: true,
        sync_series: true,
        sync_books: true,
      });

      await expect
        .poll(async () => (await getSeriesMetadata(series.seriesId)).authors)
        .toEqual([author]);
      await expect
        .poll(async () => (await getSeriesMetadata(series.seriesId)).genres)
        .toEqual([genre]);
      await expect
        .poll(async () => (await getSeriesMetadata(series.seriesId)).description)
        .toBe(description);

      // `router.refresh()` est déclenché après le PATCH : la fiche ne doit pas
      // resservir les métadonnées en cache.
      await page.keyboard.press("Escape");
      await expect(panel).toBeHidden();
      await expect(page.locator("main").getByText(author, { exact: true })).toBeVisible();
      await expect(page.locator("main").getByText(genre, { exact: true })).toBeVisible();
      await expect(page.locator("main")).toContainText(description);
    } finally {
      for (const id of linkIds) {
        await deleteMetadataLink(id).catch(() => undefined);
      }
      await removeMetadataSeries(series);
    }
  });
});
