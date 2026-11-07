import { test, expect } from "@playwright/test";

test.describe("bibliothèques — vue d'ensemble", () => {
  test("liste les bibliothèques et ouvre leurs séries", async ({ page }) => {
    await page.goto("/libraries");

    await expect(page.locator("main h1")).toContainText("Libraries");
    await expect(page.getByText("Add a library").first()).toBeVisible();

    const seriesLink = page.locator('main a[href$="/series"]').first();
    test.skip((await seriesLink.count()) === 0, "Aucune bibliothèque");

    await seriesLink.click();
    await page.waitForURL(/\/libraries\/[^/]+\/series$/);
    await expect(page.locator("main h1")).toBeVisible();
  });

  test("ouvre les livres d'une bibliothèque", async ({ page }) => {
    await page.goto("/libraries");

    const booksLink = page.locator('main a[href$="/books"]').first();
    test.skip((await booksLink.count()) === 0, "Aucune bibliothèque");

    await booksLink.click();
    await page.waitForURL(/\/libraries\/[^/]+\/books$/);
    await expect(page.locator("main h1")).toBeVisible();
  });

  test("le formulaire d'ajout de bibliothèque est présent", async ({ page }) => {
    await page.goto("/libraries");

    const addCard = page.getByText("Add a library").first().locator("xpath=ancestor::div[contains(@class,'rounded-lg')][1]");
    await expect(addCard).toBeVisible();
    await expect(addCard.getByPlaceholder("Library name")).toBeVisible();
    await expect(addCard.getByRole("button", { name: "Add library" })).toBeVisible();
  });
});
