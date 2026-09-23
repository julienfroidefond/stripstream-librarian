import { test, expect, type Page } from "@playwright/test";

const ROUTES: Array<{ path: string; name: string }> = [
  { path: "/", name: "dashboard" },
  { path: "/series", name: "series" },
  { path: "/books", name: "books" },
  { path: "/authors", name: "authors" },
  { path: "/genres", name: "genres" },
  { path: "/metadata", name: "metadata" },
  { path: "/reading-lists", name: "reading-lists" },
  { path: "/libraries", name: "libraries" },
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

test.describe("smoke — toutes les pages se rendent", () => {
  for (const route of ROUTES) {
    test(`${route.name} (${route.path})`, async ({ page }) => {
      const errors = collectPageErrors(page);

      const res = await page.goto(route.path, { waitUntil: "domcontentloaded" });
      expect(res?.status(), `HTTP ${route.path}`).toBeLessThan(400);

      await expect(page.locator("header")).toBeVisible();
      await expect(page.locator("main")).toBeVisible();

      expect(errors, `erreurs JS non capturées sur ${route.path}`).toEqual([]);
    });
  }

  test("le menu de navigation principal est présent", async ({ page }) => {
    await page.goto("/");
    const nav = page.locator("header nav");
    await expect(nav.getByRole("link")).toHaveCount(6);
    await expect(nav.getByRole("button")).toHaveCount(1);
  });

  test("le dashboard affiche des statistiques", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator("main")).toContainText(/\d/);
  });
});
