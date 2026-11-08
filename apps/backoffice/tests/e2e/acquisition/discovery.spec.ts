import { test, expect } from "@playwright/test";

import { uniqueName } from "../helpers";
import { createMetadataSeries, removeMetadataSeries } from "../support/metadata";

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

  test("la wishlist et le filtre livres exposent une série sans livre", async ({ page }) => {
    // A leading zero keeps the fixture in the first 100 title-sorted wishlist rows.
    const series = await createMetadataSeries({ name: uniqueName("000 e2e wishlist") });

    try {
      await page.goto("/discovery?tab=wishlist");
      await expect(page.locator("main h1")).toContainText("Series Discovery");
      await expect(page.getByTitle(series.name)).toBeVisible({ timeout: 30_000 });

      const query = encodeURIComponent(series.name);
      const cacheBust = uniqueName("filter").replace(/[^a-z0-9-]/gi, "");

      await page.goto(
        `/series?q=${query}&library=${series.libraryId}&books_filter=wishlist&e2e=${cacheBust}`
      );
      await expect(page.getByTitle(series.name)).toBeVisible();

      await page.goto(
        `/series?q=${query}&library=${series.libraryId}&books_filter=in_library&e2e=${cacheBust}`
      );
      await expect(page.getByText("No series found matching your filters")).toBeVisible();
    } finally {
      await removeMetadataSeries(series);
    }
  });
});
