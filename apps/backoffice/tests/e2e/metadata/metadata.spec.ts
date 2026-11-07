import { test, expect } from "@playwright/test";

test.describe("métadonnées — gaps et recherche", () => {
  test("bascule entre séries et livres", async ({ page }) => {
    await page.goto("/metadata");

    await expect(page.locator("main h1")).toContainText("Metadata");
    await expect(page.locator("main").getByText("Name", { exact: true }).first()).toBeVisible();

    await page.locator("main").getByRole("link", { name: "Books", exact: true }).click();
    await expect(page).toHaveURL(/tab=books/);
    await expect(page.locator("main").getByText("Title", { exact: true }).first()).toBeVisible();

    await page.locator("main").getByRole("link", { name: "Series", exact: true }).click();
    await expect(page).toHaveURL(/tab=series/);
    await expect(page.locator("main").getByText("Name", { exact: true }).first()).toBeVisible();
  });

  test("filtre sur un gap puis sur une recherche sans résultat", async ({ page }) => {
    await page.goto("/metadata");

    await expect(page.getByText(/results? found/).first()).toBeVisible();

    await page.locator("main").getByRole("link", { name: /No description/ }).click();
    await expect(page).toHaveURL(/gap=no_description/);
    await expect(page.getByText(/results? found/).first()).toBeVisible();

    const search = page.getByPlaceholder("Search a title, series or author…");
    await search.fill("zzz-e2e-no-match-42");
    await search.press("Enter");

    await expect(page).toHaveURL(/q=zzz-e2e-no-match-42/);
    await expect(page.getByText("Nothing matches this filter.")).toBeVisible();
  });
});
