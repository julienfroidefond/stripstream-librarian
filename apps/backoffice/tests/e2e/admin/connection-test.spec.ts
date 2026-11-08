import { test, expect } from "@playwright/test";

import { apiRequest, ApiRequestError } from "../support/api";

/**
 * Test de connexion AllDebrid depuis les réglages.
 *
 * L'endpoint est mocké : le test vérifie le branchement du bouton et le rendu
 * du résultat, pas la joignabilité réelle du service. Aucun réglage n'est
 * enregistré (le bouton est en lecture seule).
 */

test.describe("paramètres — test de connexion", () => {
  test("affiche le résultat du test AllDebrid", async ({ page }) => {
    let alldebrid: { enabled?: boolean; api_key?: string } | null = null;
    try {
      alldebrid = await apiRequest<{ enabled?: boolean; api_key?: string }>("/settings/alldebrid");
    } catch (error) {
      if (!(error instanceof ApiRequestError) || error.status !== 404) throw error;
    }
    test.skip(
      !alldebrid?.api_key || alldebrid.enabled !== true,
      "AllDebrid n'est pas configuré et activé"
    );

    await page.route("**/api/alldebrid/test", async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ success: true, message: "Connected as e2e-alldebrid" }),
      });
    });

    await page.goto("/settings?tab=downloadTools");

    const card = page.locator("main section.settings-panel").filter({
      has: page.getByRole("heading", { name: "AllDebrid", exact: true }),
    });
    await expect(card).toBeVisible();

    const testButton = card.getByRole("button", { name: "Test connection", exact: true });
    await expect(testButton).toBeEnabled();

    const [request] = await Promise.all([
      page.waitForRequest(
        (candidate) =>
          candidate.method() === "GET" &&
          new URL(candidate.url()).pathname === "/api/alldebrid/test"
      ),
      testButton.click(),
    ]);
    expect(request).toBeTruthy();

    await expect(card.getByText("Connected as e2e-alldebrid", { exact: true })).toBeVisible();
  });
});
