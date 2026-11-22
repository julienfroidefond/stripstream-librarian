import { test, expect, type Page } from "@playwright/test";

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
 * zéro, puis renommage et suppression d'un genre depuis la modale.
 *
 * La page ne lit plus `/genres/untagged-series` ni `/series?genre=` : toute la
 * liste et ses facettes viennent de `GET /genres/browse` (ADR 0028). Les
 * attentes de réponse portent donc sur ce contrat.
 */

function browseResponse(
  page: Page,
  predicate: (params: URLSearchParams) => boolean,
): Promise<unknown> {
  return page.waitForResponse((response) => {
    const url = new URL(response.url());
    if (!url.pathname.endsWith("/api/genres/browse")) return false;
    return predicate(url.searchParams);
  });
}

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

      const searchResponse = browseResponse(
        page,
        params => params.get("untagged") === "true" && params.get("q") === token,
      );
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

      // Tagging individuel d'une seule série. L'écriture se termine par la
      // relecture de `/genres/browse`, qui vide la sélection : attendre cette
      // réponse avant de cocher la série suivante.
      await firstCheckbox.check();
      await page.getByPlaceholder("Genre name to assign...").fill(manualGenre);
      const assignReload = browseResponse(
        page,
        params => params.get("untagged") === "true" && params.get("q") === token,
      );
      await page.getByRole("button", { name: "Assign to 1 series", exact: true }).click();
      await expect
        .poll(async () => (await getSeriesMetadata(first.seriesId)).genres)
        .toContain(manualGenre);
      await assignReload;

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
      const applyReload = browseResponse(
        page,
        params => params.get("untagged") === "true" && params.get("q") === token,
      );
      await page
        .getByRole("button", { name: new RegExp(`^${escapeRegExp(aiGenre)}`) })
        .click();
      await expect
        .poll(async () => (await getSeriesMetadata(second.seriesId)).genres)
        .toContain(aiGenre);
      await expect(page.getByText("Tag suggestions", { exact: true })).toHaveCount(0);
      await applyReload;
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
      const libraryResponse = browseResponse(page, params =>
        params.getAll("library_ids").includes(series.libraryId),
      );
      await page.getByRole("button", { name: METADATA_LIBRARY_NAME, exact: true }).click();
      await libraryResponse;

      const allResponse = browseResponse(
        page,
        params => !params.has("untagged") && !params.has("genres") && !params.has("exclude"),
      );
      await workspace.getByRole("button", { name: /^All\s*\(/ }).click();
      await allResponse;

      const searchResponse = browseResponse(page, params => params.get("q") === series.name);
      await workspace.getByPlaceholder("Search…").fill(series.name);
      await searchResponse;
      await expect(workspace.getByText(series.name, { exact: true })).toBeVisible();

      const genrePill = workspace.getByRole("button", {
        name: new RegExp(`^${escapeRegExp(genre)}`),
      });
      await expect(genrePill).toBeVisible();

      // Include : la série taguée reste visible.
      const includeResponse = browseResponse(page, params =>
        params.getAll("genres").includes(genre),
      );
      await genrePill.click();
      await includeResponse;
      await expect(workspace.getByText(series.name, { exact: true })).toBeVisible();

      // Exclude : la même série disparaît.
      await workspace.getByRole("button", { name: "Exclude", exact: true }).click();
      const excludeResponse = browseResponse(page, params =>
        params.getAll("exclude").includes(genre),
      );
      await genrePill.click();
      await excludeResponse;
      await expect(workspace.getByText(series.name, { exact: true })).toHaveCount(0);

      // « All » réinitialise les deux modes et réaffiche la série.
      const resetResponse = browseResponse(page, params => {
        return (
          !params.has("genres") &&
          !params.has("exclude") &&
          !params.has("q") &&
          !params.has("untagged")
        );
      });
      await workspace.getByRole("button", { name: /^All\s*\(/ }).click();
      await resetResponse;
      await expect(workspace.getByText(series.name, { exact: true })).toBeVisible();
    } finally {
      await removeMetadataSeries(series);
    }
  });

  test("renomme puis supprime un genre depuis la modale", async ({ page }) => {
    test.setTimeout(90_000);

    const genre = uniqueName("E2E Rename Genre");
    const renamed = uniqueName("E2E Renamed Genre");
    const series = await createMetadataSeries({ genres: [genre] });

    try {
      await page.goto("/genres");
      await page.getByRole("button", { name: genre, exact: true }).click();
      const modal = page.getByTestId("modal-panel");
      await expect(modal).toBeVisible();

      // Renommage : l'écriture puis la relecture de la page restent dans la
      // fenêtre busy, donc la modale n'est réarmée qu'une fois la liste à jour.
      await modal.getByTitle("Rename").click();
      await modal.getByRole("textbox").fill(renamed);
      const renameReload = browseResponse(page, params => params.get("untagged") === "true");
      await modal.getByTitle("Save").click();
      await expect
        .poll(async () => (await getSeriesMetadata(series.seriesId)).genres)
        .toContain(renamed);
      await renameReload;
      await expect(modal).toContainText(renamed);

      // Suppression : la modale se ferme après la relecture.
      page.once("dialog", dialog => dialog.accept());
      const deleteReload = browseResponse(page, params => params.get("untagged") === "true");
      await modal.getByTitle("Delete").click();
      await expect
        .poll(async () => (await getSeriesMetadata(series.seriesId)).genres)
        .not.toContain(renamed);
      await deleteReload;
      await expect(page.getByTestId("modal-panel")).toHaveCount(0);
    } finally {
      await removeMetadataSeries(series);
    }
  });
});
