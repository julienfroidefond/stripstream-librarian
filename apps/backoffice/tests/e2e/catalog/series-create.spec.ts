import { test, expect } from "@playwright/test";

import { uniqueName, waitForHydration } from "../helpers";
import { deleteSeries, getSeriesMetadata } from "../support/api";
import { ensureMetadataLibrary } from "../support/metadata";

/**
 * Création manuelle d'une série depuis la liste.
 *
 * Le premier parcours sélectionne un résultat de recherche de métadonnées
 * mocké ; le second utilise « Create without metadata ». Les deux appels
 * d'écriture (`POST /api/series/create`) passent par l'UI.
 */

test.describe("séries — création manuelle", () => {
  test("crée une série avec un résultat mocké puis une série vide", async ({ page }) => {
    test.setTimeout(90_000);

    const library = await ensureMetadataLibrary();
    const metadataTitle = uniqueName("E2E created metadata series");
    const emptyTitle = uniqueName("E2E created empty series");
    const createdIds: string[] = [];

    await page.route("**/api/metadata/search", async (route) => {
      const body = route.request().postDataJSON() as { series_name?: string };
      const isMetadataSearch = (body.series_name ?? "").startsWith("E2E created metadata");
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(
          isMetadataSearch
            ? [
                {
                  provider: "google_books",
                  external_id: "e2e-created-external",
                  external_url: "https://example.test/e2e-created",
                  title: metadataTitle,
                  authors: ["E2E Created Author"],
                  description: "E2E created description",
                  total_volumes: 3,
                  cover_url: null,
                  confidence: 0.95,
                  metadata_json: { source: "e2e" },
                },
              ]
            : []
        ),
      });
    });

    try {
      await page.goto("/series");
      await expect(page.locator("main h1")).toContainText("Series");
      await waitForHydration(page);

      // 1. Création depuis un résultat de métadonnées mocké.
      await page.getByRole("button", { name: "Add series", exact: true }).click();
      let panel = page.getByTestId("modal-panel");
      await expect(panel).toBeVisible();
      await panel.locator("select").selectOption(library.id);
      const searchInput = panel.getByPlaceholder("Search metadata...");
      await searchInput.fill("E2E created metadata");
      await expect(panel.getByRole("button", { name: new RegExp(metadataTitle) })).toBeVisible();
      await panel.getByRole("button", { name: new RegExp(metadataTitle) }).click();
      await expect(searchInput).toHaveValue(metadataTitle);

      const [createResponse] = await Promise.all([
        page.waitForResponse(
          (response) =>
            response.request().method() === "POST" &&
            new URL(response.url()).pathname === "/api/series/create"
        ),
        panel.getByRole("button", { name: "Create", exact: true }).click(),
      ]);
      expect(createResponse.ok()).toBe(true);
      await expect(page).toHaveURL(/\/series\/[0-9a-f-]+$/, { timeout: 10_000 });
      const metadataSeriesId = new URL(page.url()).pathname.split("/").pop()!;
      createdIds.push(metadataSeriesId);
      await expect(page.locator("main h1")).toHaveText(metadataTitle);

      const metadata = await getSeriesMetadata(metadataSeriesId);
      expect(metadata.authors).toEqual(["E2E Created Author"]);
      expect(metadata.description).toBe("E2E created description");
      expect(metadata.total_volumes).toBe(3);

      // 2. Création sans métadonnées.
      await page.goto("/series");
      await expect(page.locator("main h1")).toContainText("Series");
      await waitForHydration(page);
      await page.getByRole("button", { name: "Add series", exact: true }).click();
      panel = page.getByTestId("modal-panel");
      await expect(panel).toBeVisible();
      await panel.locator("select").selectOption(library.id);
      await panel.getByPlaceholder("Search metadata...").fill(emptyTitle);
      await expect(panel.getByRole("button", { name: "Create without metadata" })).toBeVisible();

      const [manualResponse] = await Promise.all([
        page.waitForResponse(
          (response) =>
            response.request().method() === "POST" &&
            new URL(response.url()).pathname === "/api/series/create"
        ),
        panel.getByRole("button", { name: "Create without metadata" }).click(),
      ]);
      expect(manualResponse.ok()).toBe(true);
      await expect(page).toHaveURL(/\/series\/[0-9a-f-]+$/, { timeout: 10_000 });
      const emptySeriesId = new URL(page.url()).pathname.split("/").pop()!;
      createdIds.push(emptySeriesId);
      await expect(page.locator("main h1")).toHaveText(emptyTitle);
    } finally {
      for (const id of createdIds) {
        await deleteSeries(id).catch(() => undefined);
      }
    }
  });
});
