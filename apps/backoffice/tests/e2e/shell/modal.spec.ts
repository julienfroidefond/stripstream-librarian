import { test, expect } from "@playwright/test";

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
