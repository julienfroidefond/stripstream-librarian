import { test, expect } from "@playwright/test";

import { escapeRegExp, uniqueName } from "../helpers";
import { getSeriesMetadata } from "../support/api";
import { createMetadataSeries, removeMetadataSeries } from "../support/metadata";

test.describe("auteurs", () => {
  test("liste, recherche et fiche auteur", async ({ page }) => {
    const author = uniqueName("E2E Author");
    const series = await createMetadataSeries({ authors: [author] });

    try {
      await page.goto(`/authors?q=${encodeURIComponent(author)}`);

      await expect(page.locator("main h1")).toContainText("Authors");
      await expect(page.getByPlaceholder("Search by author name...")).toBeVisible();

      const authorLink = page.getByRole("link", { name: new RegExp(escapeRegExp(author)) });
      await expect(authorLink).toBeVisible();
      await authorLink.click();
      await page.waitForURL(/\/authors\/[^/]+$/);

      await expect(page.locator("main h1")).toHaveText(author);
      await expect(page.getByText("0 books", { exact: true }).first()).toBeVisible();

      const seriesLink = page.getByRole("link", {
        name: new RegExp(escapeRegExp(series.name)),
      });
      await expect(seriesLink).toBeVisible();
    } finally {
      await removeMetadataSeries(series);
    }
  });

  test("une recherche sans résultat affiche l'état vide", async ({ page }) => {
    await page.goto("/authors?q=zzz-e2e-no-match-42");

    await expect(page.getByText("No authors found matching your filters")).toBeVisible();
  });
});

test.describe("genres", () => {
  test("affiche les statistiques et les filtres de genre", async ({ page }) => {
    await page.goto("/genres");

    await expect(page.locator("main h1")).toContainText("Genre management");
    await expect(page.getByText("Tag rate", { exact: true })).toBeVisible();
    await expect(page.getByText("All genres", { exact: true }).first()).toBeVisible();
    await expect(page.getByText("Top genre", { exact: true })).toBeVisible();

    await expect(page.getByRole("button", { name: "Include", exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Exclude", exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: /Series without genre/ })).toBeVisible();
  });

  test("renomme un genre depuis la modale", async ({ page }) => {
    const genre = uniqueName("E2E Genre");
    const renamed = `${genre} Renamed`;
    const series = await createMetadataSeries({ genres: [genre] });

    try {
      await page.goto("/genres");

      const genreButton = page.getByRole("button", { name: genre, exact: true });
      await expect(genreButton).toBeVisible();
      await genreButton.click();

      const panel = page.getByTestId("modal-panel");
      await expect(panel).toBeVisible();

      await panel.getByTitle("Rename").click();
      const input = panel.locator("input");
      await expect(input).toHaveValue(genre);
      await input.fill(renamed);
      await panel.getByTitle("Save").click();

      await expect(page.getByRole("button", { name: renamed, exact: true })).toBeVisible();
      await expect(page.getByRole("button", { name: genre, exact: true })).toHaveCount(0);
    } finally {
      await removeMetadataSeries(series);
    }
  });

  test("affecte un genre en masse, filtre les séries sans genre puis supprime le genre", async ({
    page,
  }) => {
    test.setTimeout(60_000);

    const token = uniqueName("E2E genre");
    const first = await createMetadataSeries({ name: `Étoile ${token} A` });
    const second = await createMetadataSeries({ name: `Étoile ${token} B` });
    const genre = uniqueName("E2E Bulk Genre");
    // La recherche de l'atelier doit être insensible à la casse et aux accents.
    const search = `etoile ${token.toLowerCase()}`;

    try {
      await page.goto("/genres");
      await expect(page.locator("main h1")).toContainText("Genre management");

      // Bascule All -> Series without genre pour charger explicitement la liste
      // des séries non taguées, puis recherche insensible aux accents.
      await page.getByRole("button", { name: /^All\s*\(/ }).click();
      await page.getByRole("button", { name: /^Series without genre/ }).click();
      await page.getByRole("button", { name: "Table", exact: true }).click();

      // La recherche est débouncée : attendre sa réponse évite qu'elle ne
      // réinitialise la sélection juste après les clics sur les cases.
      const searchResponse = page.waitForResponse((response) => {
        if (!response.url().includes("/api/genres/untagged-series")) return false;
        return new URL(response.url()).searchParams.get("q") === search;
      });
      await page.getByPlaceholder("Search…").last().fill(search);
      await searchResponse;

      for (const series of [first, second]) {
        const checkbox = page.getByRole("checkbox", { name: series.name, exact: true });
        await expect(checkbox).toBeVisible();
        await checkbox.check();
      }

      await page.getByPlaceholder("Genre name to assign...").fill(genre);
      const assign = page.getByRole("button", { name: "Assign to 2 series", exact: true });
      await expect(assign).toBeEnabled();
      await assign.click();
      await expect(page.getByText("Genre assigned to 2 series.", { exact: true })).toBeVisible();

      for (const series of [first, second]) {
        await expect
          .poll(async () => (await getSeriesMetadata(series.seriesId)).genres)
          .toContain(genre);
      }

      // Les deux séries ont quitté la vue « sans genre ».
      await expect(page.getByRole("checkbox", { name: first.name, exact: true })).toHaveCount(0);
      await expect(page.getByRole("checkbox", { name: second.name, exact: true })).toHaveCount(0);

      // La modale du genre liste les deux séries, puis le genre est supprimé
      // de toutes les séries via la confirmation native.
      const genreButton = page.getByRole("button", { name: genre, exact: true });
      await expect(genreButton).toBeVisible();
      await genreButton.click();
      const panel = page.getByTestId("modal-panel");
      await expect(panel).toBeVisible();
      await expect(panel.getByText(first.name, { exact: true })).toBeVisible();
      await expect(panel.getByText(second.name, { exact: true })).toBeVisible();

      page.once("dialog", (dialog) => dialog.accept());
      await panel.getByTitle("Delete").click();
      await expect(panel).toBeHidden();

      for (const series of [first, second]) {
        await expect
          .poll(async () => (await getSeriesMetadata(series.seriesId)).genres)
          .not.toContain(genre);
      }
      await expect(page.getByRole("button", { name: genre, exact: true })).toHaveCount(0);

      // La liste courante est rafraîchie : les séries réapparaissent dans la
      // vue « sans genre » sans rechargement manuel.
      await expect(page.getByRole("checkbox", { name: first.name, exact: true })).toBeVisible();
      await expect(page.getByRole("checkbox", { name: second.name, exact: true })).toBeVisible();
    } finally {
      await removeMetadataSeries(first);
      await removeMetadataSeries(second);
    }
  });
});
