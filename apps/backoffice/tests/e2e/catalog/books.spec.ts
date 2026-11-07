import { test, expect } from "@playwright/test";

test.describe("livres — liste et détail", () => {
  test("ouvre la fiche d'un livre et ses informations techniques", async ({ page }) => {
    await page.goto("/books");

    const firstBook = page.locator('a[href^="/books/"]').first();
    test.skip((await firstBook.count()) === 0, "Aucun livre en base");

    await firstBook.click();
    await page.waitForURL(/\/books\/[^/]+$/);

    await expect(page.locator("main h1")).toBeVisible();
    await expect(page.getByRole("link", { name: "Libraries" }).first()).toBeVisible();

    await page.locator("summary", { hasText: "Technical information" }).click();
    await expect(page.getByText("Book ID")).toBeVisible();
  });

  test("une recherche sans résultat affiche l'état vide", async ({ page }) => {
    await page.goto("/books?q=zzz-e2e-no-match-42");

    await expect(page.getByText(/No books found for/)).toBeVisible();
  });
});
