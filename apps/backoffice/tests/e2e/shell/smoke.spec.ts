import { test, expect, type Page } from "@playwright/test";

/**
 * Smoke coverage: every top-level page must render without a server error, a
 * broken layout, or an uncaught JavaScript error.
 *
 * Feature-specific assertions live in the domain folders (`dashboard/`,
 * `catalog/`, `metadata/`, `reading/`, `libraries/`, `acquisition/`,
 * `operations/`, `admin/`).
 */

const ROUTES: Array<{ path: string; name: string }> = [
  { path: "/", name: "dashboard" },
  { path: "/series", name: "series" },
  { path: "/books", name: "books" },
  { path: "/progress", name: "progress" },
  { path: "/authors", name: "authors" },
  { path: "/genres", name: "genres" },
  { path: "/metadata", name: "metadata" },
  { path: "/reading-lists", name: "reading-lists" },
  { path: "/libraries", name: "libraries" },
  { path: "/diagnostics", name: "diagnostics" },
  { path: "/discovery", name: "discovery" },
  { path: "/jobs", name: "jobs" },
  { path: "/downloads", name: "downloads" },
  { path: "/tokens", name: "tokens" },
  { path: "/settings", name: "settings" },
];

function collectPageErrors(page: Page): string[] {
  const errors: string[] = [];
  page.on("pageerror", (err) => errors.push(err.message));
  return errors;
}

test.describe("shell — toutes les pages se rendent", () => {
  for (const route of ROUTES) {
    test(`${route.name} (${route.path})`, async ({ page }) => {
      const errors = collectPageErrors(page);

      const res = await page.goto(route.path, { waitUntil: "domcontentloaded" });
      expect(res?.status(), `HTTP ${route.path}`).toBeLessThan(400);

      await expect(page.getByRole("banner")).toBeVisible();
      await expect(page.locator("main")).toBeVisible();

      expect(errors, `erreurs JS non capturées sur ${route.path}`).toEqual([]);
    });
  }
});
