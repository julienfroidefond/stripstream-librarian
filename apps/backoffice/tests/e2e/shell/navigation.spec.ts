import { test, expect } from "@playwright/test";

test.describe("shell — navigation", () => {
  test("le menu desktop expose les domaines et les sous-menus", async ({ page }) => {
    await page.goto("/");

    const header = page.locator("header");
    await expect(header).toBeVisible();
    await expect(header).toContainText("e2e-reader");

    // Catalog dropdown → Series
    await header.locator('button[title="Catalog"]').click();
    const catalogMenu = header.getByRole("menu");
    await expect(catalogMenu.getByRole("menuitem", { name: "Series" })).toBeVisible();
    await catalogMenu.getByRole("menuitem", { name: "Series" }).click();
    await expect(page).toHaveURL(/\/series$/);
    await expect(page.locator("main h1")).toContainText("Series");

    // Data dropdown → Genres
    await header.locator('button[title="Data"]').click();
    const dataMenu = header.getByRole("menu");
    await expect(dataMenu.getByRole("menuitem", { name: "Genres" })).toBeVisible();
    await dataMenu.getByRole("menuitem", { name: "Genres" }).click();
    await expect(page).toHaveURL(/\/genres$/);
    await expect(page.locator("main h1")).toContainText("Genre management");
  });

  test("les liens directs mènent aux pages de premier niveau", async ({ page }) => {
    await page.goto("/");

    for (const [title, path] of [
      ["Progress", "/progress"],
      ["Libraries", "/libraries"],
      ["Diagnostics", "/diagnostics"],
      ["Discover", "/discovery"],
    ] as const) {
      await page.locator(`header a[title="${title}"]`).click();
      await expect(page).toHaveURL(new RegExp(`${path}$`));
      await expect(page.locator("main h1")).toBeVisible();
      await page.goto("/");
    }
  });

  test("le menu mobile permet de naviguer", async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto("/");

    const openMenu = page.getByRole("button", { name: "Open menu" });
    await expect(openMenu).toBeVisible();
    await openMenu.click();

    await expect(page.getByText("Navigation", { exact: true })).toBeVisible();
    await page
      .getByRole("navigation")
      .getByRole("link", { name: "Books", exact: true })
      .click();

    await expect(page).toHaveURL(/\/books$/);
    await expect(page.locator("main h1")).toContainText("Books");
    await expect(page.getByRole("button", { name: "Open menu" })).toBeVisible();
  });

  test("le sélecteur de thème bascule le mode sombre", async ({ page }) => {
    await page.goto("/");

    const html = page.locator("html");
    const toggle = page.locator('header button[aria-label^="Switch to"]').first();
    await expect(toggle).toBeVisible();

    const before = await html.getAttribute("class");
    await toggle.click();
    await expect.poll(async () => (await html.getAttribute("class")) ?? "").not.toBe(before ?? "");
  });

  test("l'indicateur de jobs est cliquable", async ({ page }) => {
    await page.goto("/");

    const header = page.locator("header");
    const button = header.locator('button[title*="active job"]');

    if ((await button.count()) > 0) {
      await button.first().click();
      await expect(page.getByText("Active jobs")).toBeVisible();
      return;
    }

    await expect(header.locator('a[href="/jobs"]')).toBeVisible();
  });
});
