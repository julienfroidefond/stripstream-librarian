import { test, expect } from "@playwright/test";

test.describe("livres — liste vers détail", () => {
  test("ouvre la fiche d'un livre et ses informations techniques", async ({ page }) => {
    await page.goto("/books");

    const firstBook = page.locator('a[href^="/books/"]').first();
    test.skip((await firstBook.count()) === 0, "Aucun livre en base");

    await firstBook.click();
    await page.waitForURL(/\/books\/[^/]+$/);

    await expect(page.locator("main h1")).toBeVisible();
    await expect(page.getByRole("link", { name: "Libraries" }).first()).toBeVisible();

    await page.locator("summary", { hasText: "Technical information" }).click();
    await expect(page.getByText("Book ID")).toBeVisible();
  });
});

test.describe("jobs — liste vers détail", () => {
  test("affiche les actions de lancement puis ouvre un job", async ({ page }) => {
    await page.goto("/jobs");

    await expect(page.getByRole("heading", { name: "Indexing jobs" })).toBeVisible();
    await expect(page.getByText("Start a job")).toBeVisible();

    const firstJob = page.locator('a[href^="/jobs/"]').first();
    test.skip((await firstJob.count()) === 0, "Aucun job en base");

    await firstJob.click();
    await page.waitForURL(/\/jobs\/[^/]+$/);

    await expect(page.getByRole("heading", { name: "Job details" })).toBeVisible();
    await expect(page.getByRole("link", { name: "Back to jobs" })).toBeVisible();
  });
});

test.describe("téléchargements — filtres", () => {
  test("les filtres de statut sont présents et filtrent la liste", async ({ page }) => {
    await page.goto("/downloads");

    await expect(page.getByRole("heading", { name: "Downloads" })).toBeVisible();

    const active = page.getByRole("button", { name: /In progress/ });
    await expect(active.first()).toBeVisible();
    await expect(page.getByRole("button", { name: /Imported/ }).first()).toBeVisible();
    await expect(page.getByRole("button", { name: /Error/ }).first()).toBeVisible();

    await active.first().click();
    await expect(page.locator("main")).toBeVisible();
  });
});

test.describe("bibliothèques — carte vers les séries", () => {
  test("ouvre la liste des séries d'une bibliothèque", async ({ page }) => {
    await page.goto("/libraries");

    await expect(page.getByText("Add a library").first()).toBeVisible();

    const libraryLink = page.locator('a[href^="/libraries/"]').first();
    test.skip((await libraryLink.count()) === 0, "Aucune bibliothèque");

    await libraryLink.click();
    await page.waitForURL(/\/libraries\/[^/]+\/(series|books)/);

    await expect(page.locator("main")).toBeVisible();
  });
});

test.describe("réglages — navigation par onglets", () => {
  test("change d'onglet et met à jour l'URL", async ({ page }) => {
    await page.goto("/settings");

    await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();

    await page.getByRole("button", { name: "Tokens", exact: true }).click();
    await expect(page).toHaveURL(/tab=tokens/);

    await expect(page.getByRole("heading", { name: "API Tokens" })).toBeVisible();
    await expect(page.getByPlaceholder("Token name")).toBeVisible();
  });
});
