import { test, expect, type Page } from "@playwright/test";

import { uniqueName, waitForHydration } from "../helpers";
import { createDisposableBook, removeDisposableBook } from "../support/books";
import {
  createMetadataSeries,
  ensureMetadataLibrary,
  removeMetadataSeries,
  type MetadataSeries,
} from "../support/metadata";

/** Wait until the debounced filter navigation has written the expected param. */
async function waitForParam(page: Page, name: string, value: string): Promise<void> {
  await page.waitForURL((url) => url.searchParams.get(name) === value, { timeout: 10_000 });
}

async function waitForNoParam(page: Page, name: string): Promise<void> {
  await page.waitForURL((url) => !url.searchParams.has(name), { timeout: 10_000 });
}

async function expectCardTitles(page: Page, prefix: string, count: number): Promise<void> {
  await expect(page.locator(`main h3[title^="${prefix}"]`)).toHaveCount(count);
}

test.describe("listes catalogue — contrôles", () => {
  test("filtre, trie, pagine, bascule et navigue dans les séries", async ({ page }) => {
    test.setTimeout(90_000);

    const library = await ensureMetadataLibrary();
    const prefix = uniqueName("E2E list series");
    const fixtures: MetadataSeries[] = [];

    try {
      for (let index = 0; index < 5; index += 1) {
        fixtures.push(
          await createMetadataSeries({ name: `${prefix} ${String(index).padStart(2, "0")}` })
        );
      }

      await page.goto("/series");
      await expect(page.locator("main h1")).toContainText("Series");
      await waitForHydration(page);

      // Grille ↔ liste : le cookie pilote le rendu serveur.
      await page.getByRole("button", { name: "List" }).click();
      await expect(page.getByRole("button", { name: "Grid" })).toBeVisible();
      await expect
        .poll(async () => (await page.context().cookies()).find((c) => c.name === "view_mode")?.value)
        .toBe("list");
      await page.getByRole("button", { name: "Grid" }).click();
      await expect(page.getByRole("button", { name: "List" })).toBeVisible();

      // Onglets de regroupement et de recommandations.
      await page.getByRole("link", { name: "Reading lists", exact: true }).click();
      await expect(page).toHaveURL(/group_by=reading_list/);
      await page.getByRole("link", { name: "Recommendations", exact: true }).click();
      await expect(page).toHaveURL(/view=recommendations/);
      await expect(page.locator("main h2", { hasText: "Recommendations" })).toBeVisible();
      await page.getByRole("link", { name: "All series", exact: true }).click();
      await expect(page).toHaveURL(/\/series$/);

      // Texte + bibliothèque : les deux filtres se combinent dans l'URL.
      await page.getByPlaceholder("Search by series name...").fill(prefix);
      await waitForParam(page, "q", prefix);
      await page.reload();
      await waitForHydration(page);
      await page.getByLabel("Library").selectOption(library.id);
      await waitForParam(page, "library", library.id);
      await page.reload();
      await waitForHydration(page);
      await expect(page.getByText(/^5 series/)).toBeVisible();
      await expectCardTitles(page, prefix, 5);

      // Les selects exposent leurs libellés malgré la pastille visuelle.
      await page.getByLabel("Sort").selectOption("latest");
      await waitForParam(page, "sort", "latest");
      await page.reload();
      await waitForHydration(page);
      await expectCardTitles(page, prefix, 5);

      await page.getByLabel("Metadata").selectOption("unlinked");
      await waitForParam(page, "metadata_provider", "unlinked");
      await page.reload();
      await waitForHydration(page);
      await expectCardTitles(page, prefix, 5);

      // Un filtre sans correspondance doit afficher l'état vide.
      await page.getByLabel("Status", { exact: true }).selectOption("ongoing");
      await waitForParam(page, "series_status", "ongoing");
      await expect(page.getByText("No series found matching your filters")).toBeVisible();
      await page.getByLabel("Status", { exact: true }).selectOption({ label: "All statuses" });
      await waitForNoParam(page, "series_status");
      await page.reload();
      await waitForHydration(page);
      await expectCardTitles(page, prefix, 5);

      // “Clear” efface tous les filtres via le formulaire.
      await page.locator('form button:has-text("Clear")').click();
      await expect(page).toHaveURL(/\/series$/);
      await page.reload();
      await waitForHydration(page);
      await expect(page.getByText(/series matching/)).toHaveCount(0);

      // Pagination : limite réduite dans l'URL puis navigation via les contrôles.
      await page.goto(
        `/series?q=${encodeURIComponent(prefix)}&library=${library.id}&limit=2`
      );
      await waitForHydration(page);
      await expectCardTitles(page, prefix, 2);
      await expect(page.getByText("1-2 of 5")).toBeVisible();
      await page.getByTitle("Next page").click();
      await waitForParam(page, "page", "2");
      await page.reload();
      await waitForHydration(page);
      await expectCardTitles(page, prefix, 2);
      await expect(page.getByText("3-4 of 5")).toBeVisible();

      // Taille de page via le select : la pagination revient à la page 1.
      await page.getByLabel("per page").selectOption("48");
      await waitForParam(page, "limit", "48");
      await page.reload();
      await waitForHydration(page);
      await expectCardTitles(page, prefix, 5);
      await expect(page.getByText("1-5 of 5")).toBeVisible();
    } finally {
      await Promise.all(fixtures.map((fixture) => removeMetadataSeries(fixture)));
    }
  });

  test("filtre, trie et bascule la liste des livres", async ({ page }) => {
    test.setTimeout(90_000);

    const book = await createDisposableBook();
    try {
      await page.goto("/books");
      await expect(page.locator("main h1")).toContainText("Books");
      await waitForHydration(page);

      await page.getByPlaceholder("Search by title, author, series...").fill(book.title);
      await waitForParam(page, "q", book.title);
      await page.getByLabel("Library").selectOption(book.libraryId);
      await waitForParam(page, "library", book.libraryId);
      await page.getByLabel("Format").selectOption("cbz");
      await waitForParam(page, "format", "cbz");
      await page.getByLabel("Metadata").selectOption("unlinked");
      await waitForParam(page, "metadata", "unlinked");
      await page.getByLabel("Sort").selectOption("latest");
      await waitForParam(page, "sort", "latest");

      // Finish the pending filter refresh before toggling the view: the toggle
      // starts its own refresh and a click on a form being replaced is lost.
      await page.reload();
      await waitForHydration(page);
      await expect(page.locator("main h3").filter({ hasText: book.title }).first()).toBeVisible();

      await page.getByRole("button", { name: "List" }).click();
      await expect(page.getByRole("button", { name: "Grid" })).toBeVisible();

      await page.locator('form button:has-text("Clear")').click();
      await expect(page).toHaveURL(/\/books$/);
    } finally {
      await removeDisposableBook(book);
    }
  });

  test("recherche et trie les auteurs", async ({ page }) => {
    const author = uniqueName("E2E list author");
    const series = await createMetadataSeries({ authors: [author] });

    try {
      await page.goto("/authors");
      await expect(page.locator("main h1")).toContainText("Authors");
      await waitForHydration(page);

      await page.getByPlaceholder("Search by author name...").fill(author);
      await waitForParam(page, "q", author);
      await expect(page.getByRole("link", { name: new RegExp(author) }).first()).toBeVisible();

      await page.getByLabel("Sort").selectOption("books");
      await waitForParam(page, "sort", "books");

      await page.reload();
      await waitForHydration(page);
      await page.locator('form button:has-text("Clear")').click();
      await expect(page).toHaveURL(/\/authors$/);
    } finally {
      await removeMetadataSeries(series);
    }
  });

  test("filtre et change d'onglet dans les lacunes de métadonnées", async ({ page }) => {
    const library = await ensureMetadataLibrary();
    const prefix = uniqueName("E2E metadata gap");
    const series = await createMetadataSeries({ name: prefix });

    try {
      await page.goto(`/metadata?tab=series&q=${encodeURIComponent(prefix)}`);
      await expect(page.locator("main h1")).toContainText("Metadata");
      await waitForHydration(page);
      await expect(page.getByRole("cell", { name: series.name })).toBeVisible();

      await page.getByLabel("Library").selectOption(library.id);
      await waitForParam(page, "library", library.id);
      await expect(page.getByRole("cell", { name: series.name })).toBeVisible();

      await page.reload();
      await waitForHydration(page);
      await page.locator('form button:has-text("Clear")').click();
      await expect(page).toHaveURL(/tab=series/);
      await expect(page).not.toHaveURL(/library=/);
      await expect(page.locator('form button:has-text("Clear")')).toHaveCount(0);
      // Leave the server transition started by "Clear" finish before navigating.
      await page.waitForTimeout(500);

      await page.getByRole("link", { name: "Books", exact: true }).click();
      await expect(page).toHaveURL(/tab=books/);
    } finally {
      await removeMetadataSeries(series);
    }
  });
});
