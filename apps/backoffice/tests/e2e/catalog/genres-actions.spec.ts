import { test, expect } from "@playwright/test";

import { escapeRegExp, uniqueName } from "../helpers";
import { getSeriesMetadata } from "../support/api";
import {
  createMetadataSeries,
  METADATA_LIBRARY_NAME,
  removeMetadataSeries,
} from "../support/metadata";

/**
 * Atelier de genres : tagging individuel, sélection globale, suggestions IA
 * mockées, filtre par bibliothèque et bascule Include/Exclude avec remise à
 * zéro. Les fixtures sont des séries vides de `e2e-metadata`.
 */

test.describe("genres — actions de l'atelier", () => {
  test("tag individuel, tout sélectionner/désélectionner et suggestion IA", async ({ page }) => {
    test.setTimeout(90_000);

    const token = uniqueName("E2E genre actions");
    const first = await createMetadataSeries({ name: `${token} A` });
    const second = await createMetadataSeries({ name: `${token} B` });
    const manualGenre = uniqueName("E2E Manual Genre");
    const aiGenre = uniqueName("E2E AI Genre");

    try {
      await page.goto("/genres");
      await expect(page.locator("main h1")).toContainText("Genre management");
      await page.getByRole("button", { name: /^Series without genre/ }).click();
      await page.getByRole("button", { name: "Table", exact: true }).click();

      const searchResponse = page.waitForResponse((response) => {
        if (!response.url().includes("/api/genres/untagged-series")) return false;
        return new URL(response.url()).searchParams.get("q") === token;
      });
      await page.getByPlaceholder("Search…").last().fill(token);
      await searchResponse;

      const firstCheckbox = page.getByRole("checkbox", { name: first.name, exact: true });
      const secondCheckbox = page.getByRole("checkbox", { name: second.name, exact: true });
      await expect(firstCheckbox).toBeVisible();
      await expect(secondCheckbox).toBeVisible();

      // Sélection globale puis désélection globale.
      await page.getByRole("button", { name: "Select all", exact: true }).click();
      await expect(page.getByRole("button", { name: "Deselect all", exact: true })).toBeVisible();
      await expect(page.getByText("2 sél.", { exact: true })).toBeVisible();
      await page.getByRole("button", { name: "Deselect all", exact: true }).click();
      await expect(page.getByText("2 sél.", { exact: true })).toHaveCount(0);

      // Tagging individuel d'une seule série.
      await firstCheckbox.check();
      await page.getByPlaceholder("Genre name to assign...").fill(manualGenre);
      await page.getByRole("button", { name: "Assign to 1 series", exact: true }).click();
      await expect
        .poll(async () => (await getSeriesMetadata(first.seriesId)).genres)
        .toContain(manualGenre);

      // Suggestion IA mockée : aucun appel au provider réel.
      await secondCheckbox.check();
      await page.route("**/api/genres/ai-suggest", async (route) => {
        await route.fulfill({
          json: {
            suggestions: [{ series_id: second.seriesId, name: second.name, tags: [aiGenre] }],
          },
        });
      });
      await page.getByRole("button", { name: "Suggest for 1 series", exact: true }).click();
      await expect(page.getByText("Tag suggestions", { exact: true })).toBeVisible();
      await page
        .getByRole("button", { name: new RegExp(`^${escapeRegExp(aiGenre)}`) })
        .click();
      await expect
        .poll(async () => (await getSeriesMetadata(second.seriesId)).genres)
        .toContain(aiGenre);
      await expect(page.getByText("Tag suggestions", { exact: true })).toHaveCount(0);
    } finally {
      await removeMetadataSeries(first);
      await removeMetadataSeries(second);
    }
  });

  test("filtre par bibliothèque, Include/Exclude et remise à zéro", async ({ page }) => {
    test.setTimeout(90_000);

    const genre = uniqueName("E2E Filter Genre");
    const series = await createMetadataSeries({ genres: [genre] });

    try {
      await page.goto("/genres");
      const workspace = page.locator("main section").last();
      await workspace.getByRole("button", { name: "Table", exact: true }).click();

      // Filtre par bibliothèque : la requête de genres est bien scopée.
      const libraryResponse = page.waitForResponse(
        (response) =>
          response.url().includes("/api/genres?library_id=") && response.request().method() === "GET"
      );
      await page.getByRole("button", { name: METADATA_LIBRARY_NAME, exact: true }).click();
      await libraryResponse;

      await workspace.getByRole("button", { name: /^All\s*\(/ }).click();
      const searchResponse = page.waitForResponse((response) => {
        if (!response.url().includes("/api/series?")) return false;
        return new URL(response.url()).searchParams.get("q") === series.name;
      });
      await workspace.getByPlaceholder("Search…").fill(series.name);
      await searchResponse;
      await expect(workspace.getByText(series.name, { exact: true })).toBeVisible();

      const genrePill = workspace.getByRole("button", {
        name: new RegExp(`^${escapeRegExp(genre)}`),
      });
      await expect(genrePill).toBeVisible();

      // Include : la série taguée reste visible.
      const includeResponse = page.waitForResponse((response) => {
        if (!response.url().includes("/api/series?")) return false;
        return new URL(response.url()).searchParams.get("genre") === genre;
      });
      await genrePill.click();
      await includeResponse;
      await expect(workspace.getByText(series.name, { exact: true })).toBeVisible();

      // Exclude : la même série disparaît.
      await workspace.getByRole("button", { name: "Exclude", exact: true }).click();
      const excludeResponse = page.waitForResponse((response) => {
        if (!response.url().includes("/api/series?")) return false;
        return new URL(response.url()).searchParams.get("genre") === genre;
      });
      await genrePill.click();
      await excludeResponse;
      await expect(workspace.getByText(series.name, { exact: true })).toHaveCount(0);

      // « All » réinitialise les deux modes et réaffiche la série.
      const resetResponse = page.waitForResponse((response) => {
        if (!response.url().includes("/api/series?")) return false;
        const url = new URL(response.url());
        return !url.searchParams.has("genre") && !url.searchParams.has("q");
      });
      await workspace.getByRole("button", { name: /^All\s*\(/ }).click();
      await resetResponse;
      await expect(workspace.getByText(series.name, { exact: true })).toBeVisible();
    } finally {
      await removeMetadataSeries(series);
    }
  });
});
