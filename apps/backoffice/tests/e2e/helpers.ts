import { expect, type Page } from "@playwright/test";

/** Unique per-run name so parallel workers never collide. */
export function uniqueName(prefix: string): string {
  const random = Math.random().toString(36).slice(2, 8);
  return `${prefix}-${Date.now().toString(36)}-${random}`;
}

export function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/**
 * Wait until React has hydrated a server-rendered control before interacting.
 *
 * Playwright can resolve and act on form controls as soon as the HTML streams
 * in, before React attaches their event handlers. A `selectOption` landing in
 * that window changes the DOM without triggering the filter navigation, which
 * makes URL assertions hang. React adds internal props/fiber keys on hydration.
 */
export async function waitForHydration(page: Page, selector = "form"): Promise<void> {
  await page.waitForFunction(
    (sel) => {
      const element = document.querySelector(sel);
      if (!element) return false;
      return Object.keys(element).some(
        (key) => key.startsWith("__reactProps$") || key.startsWith("__reactFiber$")
      );
    },
    selector,
    { timeout: 15_000 }
  );
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
