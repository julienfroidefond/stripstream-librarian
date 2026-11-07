import { expect, type Page } from "@playwright/test";

/** Navigate to the first series detail page and return its id. */
export async function openFirstSeries(page: Page): Promise<string> {
  await page.goto("/series");
  const firstCard = page.locator('a[href^="/series/"]').first();
  await expect(firstCard).toBeVisible();
  const href = await firstCard.getAttribute("href");
  const id = href!.split("/").pop()!;
  await page.goto(`/series/${id}`);
  await expect(page.locator("main h1")).toBeVisible();
  return id;
}

/** Unique per-run name so parallel workers never collide. */
export function uniqueName(prefix: string): string {
  const random = Math.random().toString(36).slice(2, 8);
  return `${prefix}-${Date.now().toString(36)}-${random}`;
}

export function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
