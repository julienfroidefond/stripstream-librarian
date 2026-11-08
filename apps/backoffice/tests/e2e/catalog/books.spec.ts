import { test, expect } from "@playwright/test";

import { createDisposableBook, removeDisposableBook } from "../support/books";

test.describe("livres — liste et détail", () => {
  test("ouvre la fiche d'un livre et ses informations techniques", async ({ page }) => {
    // Le livre est créé pour ce test : le premier résultat de la liste peut
    // appartenir à une spec parallèle et disparaître pendant la navigation.
    test.setTimeout(120_000);

    const book = await createDisposableBook();

    try {
      await page.goto(`/books?q=${encodeURIComponent(book.title)}`);

      const card = page.locator(`a[href="/books/${book.id}"]`).first();
      await expect(card).toBeVisible();
      await card.click();
      await page.waitForURL(new RegExp(`/books/${book.id}$`));

      await expect(page.locator("main h1")).toHaveText(book.title);
      await expect(page.getByRole("link", { name: "Libraries" }).first()).toBeVisible();

      await page.locator("summary", { hasText: "Technical information" }).click();
      await expect(page.getByText("Book ID")).toBeVisible();
    } finally {
      await removeDisposableBook(book);
    }
  });

  test("une recherche sans résultat affiche l'état vide", async ({ page }) => {
    await page.goto("/books?q=zzz-e2e-no-match-42");

    await expect(page.getByText(/No books found for/)).toBeVisible();
  });
});
