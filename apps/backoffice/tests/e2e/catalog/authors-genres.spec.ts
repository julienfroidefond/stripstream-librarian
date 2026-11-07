import { test, expect } from "@playwright/test";

test.describe("auteurs", () => {
  test("liste, recherche et fiche auteur", async ({ page }) => {
    await page.goto("/authors");

    await expect(page.locator("main h1")).toContainText("Authors");
    await expect(page.getByPlaceholder("Search by author name...")).toBeVisible();

    const firstAuthor = page.locator('a[href^="/authors/"]').first();
    test.skip((await firstAuthor.count()) === 0, "Aucun auteur en base");

    await firstAuthor.click();
    await page.waitForURL(/\/authors\/[^/]+$/);

    await expect(page.locator("main h1")).toBeVisible();
    await expect(page.getByText(/\d+ books?/).first()).toBeVisible();
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
});
