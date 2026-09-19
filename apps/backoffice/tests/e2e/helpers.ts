import { expect, type Page } from "@playwright/test";

/** Navigate to the first series detail page and return its id. */
export async function openFirstSeries(page: Page): Promise<string> {
  await page.goto("/series");
  const firstCard = page.locator('a[href^="/series/"]').first();
  await expect(firstCard).toBeVisible();
  const href = await firstCard.getAttribute("href");
  const id = href!.split("/").pop()!;
  await page.goto(`/series/${id}`);
  await expect(page.locator("main")).toBeVisible();
  return id;
}
