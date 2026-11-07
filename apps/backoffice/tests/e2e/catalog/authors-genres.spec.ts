import { test, expect } from "@playwright/test";

import { escapeRegExp, uniqueName } from "../helpers";
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
});
