import { test, expect } from "@playwright/test";

import { uniqueName, waitForHydration } from "../helpers";
import { createDisposableBook, removeDisposableBook } from "../support/books";

test.describe("shell — cas limites", () => {
  test("la modale de création valide le nom, piège le focus et le restaure", async ({ page }) => {
    await page.route("**/api/metadata/search", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: "[]",
      })
    );

    await page.goto("/series");
    await expect(page.locator("main h1")).toContainText("Series");
    await waitForHydration(page);

    const trigger = page.getByRole("button", { name: "Add series", exact: true });
    await trigger.click();

    const panel = page.getByTestId("modal-panel");
    const search = panel.getByPlaceholder("Search metadata...");
    const create = panel.getByRole("button", { name: "Create", exact: true });
    await expect(panel).toBeVisible();
    await expect(search).toBeFocused();

    // Le bouton reste désactivé tant que le nom est vide ou composé d'espaces.
    await expect(create).toBeDisabled();
    await search.fill("   ");
    await expect(create).toBeDisabled();
    await search.fill(uniqueName("E2E validation"));
    await expect(create).toBeEnabled();

    // Tab depuis le dernier contrôle revient au premier (fermeture).
    await create.focus();
    await page.keyboard.press("Tab");
    const close = page.getByTestId("modal-close");
    await expect(close).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(create).toBeFocused();

    await page.keyboard.press("Escape");
    await expect(panel).toBeHidden();
    await expect(trigger).toBeFocused();
  });

  test("une erreur d'enregistrement reste affichée dans la modale", async ({ page }) => {
    const book = await createDisposableBook();

    try {
      await page.route(`**/api/books/${book.id}`, async (route) => {
        if (route.request().method() !== "PATCH") {
          await route.continue();
          return;
        }
        await route.fulfill({
          status: 500,
          contentType: "application/json",
          body: JSON.stringify({ error: "Forced E2E save failure" }),
        });
      });

      await page.goto(`/books/${book.id}`);
      await expect(page.locator("main h1")).toHaveText(book.title);
      await page.getByRole("button", { name: /Edit metadata/ }).click();

      const panel = page.getByTestId("modal-panel");
      await expect(panel).toBeVisible();
      await panel.getByRole("button", { name: "Save", exact: true }).click();

      await expect(panel.getByText("Forced E2E save failure")).toBeVisible();
      await expect(panel).toBeVisible();
    } finally {
      await removeDisposableBook(book);
    }
  });

  test("replie les filtres sur mobile et les laisse visibles sur desktop", async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto("/series");
    await expect(page.locator("main h1")).toContainText("Series");
    await waitForHydration(page);

    const filtersCheckbox = page.getByLabel("Filters");
    const libraryFilter = page.getByLabel("Library");
    await expect(filtersCheckbox).not.toBeChecked();
    await expect(libraryFilter).toBeHidden();

    await page.getByText("Filters", { exact: true }).click();
    await expect(filtersCheckbox).toBeChecked();
    await expect(libraryFilter).toBeVisible();

    await page.getByText("Filters", { exact: true }).click();
    await expect(libraryFilter).toBeHidden();

    await page.setViewportSize({ width: 1280, height: 800 });
    await page.reload();
    await waitForHydration(page);
    await expect(page.getByText("Filters", { exact: true })).toBeHidden();
    await expect(libraryFilter).toBeVisible();
  });
});
