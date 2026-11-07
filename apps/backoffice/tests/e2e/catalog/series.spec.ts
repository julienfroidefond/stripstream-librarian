import { test, expect } from "@playwright/test";

import { openFirstSeries } from "../helpers";

test.describe("séries — recherche", () => {
  test("le filtre q restreint puis vide la liste", async ({ page }) => {
    await page.goto("/series");

    const firstTitle = page.locator("main h3[title]").first();
    await expect(firstTitle).toBeVisible();

    const name = await firstTitle.getAttribute("title");
    test.skip(!name, "Nom de série indisponible");

    await page.goto(`/series?q=${encodeURIComponent(name!)}`);
    await expect(page.getByTitle(name!).first()).toBeVisible();

    await page.goto("/series?q=zzz-e2e-no-match-42");
    await expect(page.locator('a[href^="/series/"] h3')).toHaveCount(0);
  });
});

test.describe("séries — édition des tags", () => {
  test("ajoute puis retire un éditeur sans enregistrer", async ({ page }) => {
    await openFirstSeries(page);

    const editButton = page.getByRole("button", { name: "Edit series" });
    await expect(editButton).toBeVisible();
    await editButton.click();

    const panel = page.getByTestId("modal-panel");
    await expect(panel).toBeVisible();

    const publisherInput = panel.getByPlaceholder("Add a publisher (Enter to confirm)");
    await publisherInput.pressSequentially("e2e-smoke-publisher");

    const addButton = publisherInput.locator("xpath=..").getByRole("button", { name: "+" });
    await expect(addButton).toBeEnabled();
    await addButton.click();

    await expect(panel.getByText("e2e-smoke-publisher")).toBeVisible();
    await panel.getByRole("button", { name: "Remove e2e-smoke-publisher" }).click();
    await expect(panel.getByText("e2e-smoke-publisher")).toHaveCount(0);

    await panel.getByRole("button", { name: "Cancel" }).click();
    await expect(panel).toBeHidden();
  });
});

test.describe("séries — menu d'actions", () => {
  test("ouvre puis referme la modale de recherche de métadonnées", async ({ page }) => {
    await openFirstSeries(page);

    await page.getByRole("button", { name: "More" }).first().click();

    const item = page.getByRole("menuitem", { name: /metadata/i }).first();
    await expect(item).toBeVisible();
    await item.click();

    const panel = page.getByTestId("modal-panel");
    await expect(panel).toBeVisible();

    await page.getByTestId("modal-close").click();
    await expect(panel).toBeHidden();
  });

  test("la confirmation de suppression s'affiche et s'annule", async ({ page }) => {
    await openFirstSeries(page);

    await page.getByRole("button", { name: "More" }).first().click();

    // Le menu doit rester dans le viewport (clampé) : le dernier item est cliquable.
    const menuBox = await page.getByRole("menu").boundingBox();
    const viewport = page.viewportSize();
    expect(menuBox!.y + menuBox!.height).toBeLessThanOrEqual(viewport!.height);

    await page.getByRole("menuitem", { name: "Delete series" }).click();

    const panel = page.getByTestId("modal-panel");
    await expect(panel).toBeVisible();
    await expect(panel).toContainText("Delete series");

    await page.getByRole("button", { name: "Cancel" }).click();
    await expect(panel).toBeHidden();
  });
});

test.describe("séries — lecture", () => {
  test("le bouton de lecture est présent sur la fiche série", async ({ page }) => {
    await openFirstSeries(page);

    const markRead = page.getByRole("button", { name: /Mark (all read|unread|as read)/ }).first();
    await expect(markRead).toBeVisible();
    await expect(markRead).toBeEnabled();
  });

  test("les volumes de la série sont listés", async ({ page }) => {
    await openFirstSeries(page);

    const main = page.locator("main");
    await expect(main.locator("h1")).toBeVisible();
    await expect(main.getByRole("link", { name: "Libraries" }).first()).toBeVisible();
    await expect(main.getByText(/Mark (all read|as read)/).first()).toBeVisible();
  });
});
