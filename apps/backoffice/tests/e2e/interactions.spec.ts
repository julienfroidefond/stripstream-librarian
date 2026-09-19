import { test, expect } from "@playwright/test";

import { openFirstSeries } from "./helpers";

test.describe("ui/Modal — ouverture, fermeture, Escape", () => {
  test("la modale s'ouvre, se ferme par Escape puis par le bouton", async ({ page }) => {
    await page.goto("/series");

    await page.getByRole("button", { name: "Add series" }).click();

    const panel = page.getByTestId("modal-panel");
    await expect(panel).toBeVisible();
    await expect(panel).toContainText("New series");

    await page.keyboard.press("Escape");
    await expect(panel).toBeHidden();

    await page.getByRole("button", { name: "Add series" }).click();
    await expect(panel).toBeVisible();
    await page.getByTestId("modal-close").click();
    await expect(panel).toBeHidden();
  });
});

test.describe("DeleteConfirmButton — la confirmation s'affiche sans supprimer", () => {
  test("ouvre la modale de confirmation et annule", async ({ page }) => {
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

test.describe("MarkReadButton — rendu sur la fiche série", () => {
  test("le bouton est présent avec un libellé d'action", async ({ page }) => {
    await openFirstSeries(page);

    const markRead = page.getByRole("button", { name: /Mark (all read|unread|as read)/ }).first();
    await expect(markRead).toBeVisible();
    await expect(markRead).toBeEnabled();
  });
});

test.describe("TestConnectionButton — présent dans les réglages", () => {
  test("au moins un bouton de test de connexion est rendu", async ({ page }) => {
    await page.goto("/settings?tab=downloadTools");

    const buttons = page.getByRole("button", { name: "Test connection" });
    await expect(buttons.first()).toBeVisible();
    expect(await buttons.count()).toBeGreaterThan(0);
  });
});

test.describe("JobsIndicator — pastille de jobs", () => {
  test("affiche le lien ou la popin des jobs actifs", async ({ page }) => {
    await page.goto("/");

    const linked = page.locator('header a[href="/jobs"]');
    const button = page.locator('header button[title*="active job"]');

    if ((await button.count()) > 0) {
      await button.first().click();
      await expect(page.getByText("Active jobs")).toBeVisible();
    } else {
      await expect(linked).toBeVisible();
    }
  });
});
