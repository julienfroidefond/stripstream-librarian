import { test, expect } from "@playwright/test";

import { openFirstSeries } from "./helpers";

test.describe("liste des séries — recherche", () => {
  test("le filtre q restreint puis vide la liste", async ({ page }) => {
    await page.goto("/series");

    const firstHeading = page.locator('a[href^="/series/"] h3').first();
    test.skip((await firstHeading.count()) === 0, "Aucune série en base");

    const name = await firstHeading.getAttribute("title");
    test.skip(!name, "Nom de série indisponible");

    await page.goto(`/series?q=${encodeURIComponent(name!)}`);
    await expect(page.getByTitle(name!).first()).toBeVisible();

    await page.goto("/series?q=zzz-e2e-no-match-42");
    await expect(page.locator('a[href^="/series/"] h3')).toHaveCount(0);
  });
});

test.describe("EditSeriesForm — gestion des tags", () => {
  test("ajoute puis retire un éditeur sans enregistrer", async ({ page }) => {
    await openFirstSeries(page);

    const editButton = page.getByRole("button", { name: "Edit series" });
    test.skip((await editButton.count()) === 0, "Formulaire d'édition indisponible");
    await editButton.first().click();

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

test.describe("Menu d'actions — ouverture d'une modale", () => {
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
});
