import { test, expect } from "@playwright/test";

test.describe("découverte de séries", () => {
  test("affiche les suggestions et bascule sur la wishlist", async ({ page }) => {
    await page.goto("/discovery");

    await expect(page.locator("main h1")).toContainText("Series Discovery");

    const wishlistTab = page.getByRole("button", { name: "Wishlist" });
    await expect(wishlistTab).toBeVisible();
    await wishlistTab.click();

    await expect(page).toHaveURL(/tab=wishlist/);
    await expect(
      page.getByText(/Your wishlist is empty|series without books/).first()
    ).toBeVisible();
  });

  test("le panneau des séries masquées s'ouvre et se ferme", async ({ page }) => {
    await page.goto("/discovery");

    const hiddenPanel = page.getByRole("heading", { name: "Hidden series" });
    await expect(hiddenPanel).toBeHidden();

    await page.locator('button[title="Hidden series"]').click();

    await expect(hiddenPanel).toBeVisible();
    await expect(page.getByText("No hidden series")).toBeVisible();
  });
});
