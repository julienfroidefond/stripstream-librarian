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

/**
 * Open a fixture series through the list.
 *
 * Temporary libraries are recreated on every run with a fresh id, while the
 * series detail page resolves its library through `fetchLibraries` (Next cache,
 * 30 s). Going through the list warms that cache; retrying tolerates a cold dev
 * compile and a stale cache left by a previous run.
 */
export async function openSeriesDetail(
  page: Page,
  target: { name: string; seriesId: string; libraryName: string }
): Promise<void> {
  const listUrl = `/series?q=${encodeURIComponent(target.name)}&e2e=${encodeURIComponent(
    uniqueName("detail")
  )}`;
  await expect(async () => {
    await page.goto(listUrl);
    await expect(page.getByTitle(target.name)).toBeVisible();
    await expect(
      page.locator('select[name="library"] option', { hasText: target.libraryName })
    ).toHaveCount(1, { timeout: 1500 });
  }).toPass({ timeout: 20_000 });

  await page.getByRole("link", { name: target.name, exact: true }).click();
  await expect(page).toHaveURL(new RegExp(`/series/${escapeRegExp(target.seriesId)}$`));
  await expect(page.locator("main h1")).toHaveText(target.name);
}
