import { test, expect, type Locator, type Page } from "@playwright/test";

import { apiRequest, ApiRequestError } from "../support/api";

/**
 * Tests de connexion depuis les réglages.
 *
 * Les endpoints sont mockés : les tests vérifient le branchement des boutons et
 * le rendu du résultat, pas la joignabilité réelle des services. Les
 * enregistrements de réglages sont interceptés pour ne jamais modifier la
 * configuration globale.
 */

/** Block every settings write while the card only needs local UI state. */
async function mockSettingsWrites(page: Page): Promise<void> {
  await page.route("**/api/settings/**", async (route) => {
    if (route.request().method() !== "POST") {
      await route.continue();
      return;
    }
    // Match the real proxy: the written value is echoed back and adopted as the
    // draft's acknowledged baseline.
    const body = route.request().postDataJSON() as { value: unknown };
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(body.value),
    });
  });
}

function settingsCard(page: Page, title: string): Locator {
  return page.locator("main section.settings-panel").filter({
    has: page.getByRole("heading", { name: title, exact: true }),
  });
}

async function ensureEnabled(card: Locator): Promise<void> {
  // `Switch` renders a visually hidden checkbox, not an ARIA switch.
  const toggle = card.getByRole("checkbox").first();
  if (!(await toggle.isChecked())) {
    // The checkbox is visually replaced by a peer-styled span; force the check
    // instead of trying to click the hidden input.
    await toggle.check({ force: true });
    await expect(toggle).toBeChecked();
  }
}

test.describe("paramètres — test de connexion", () => {
  test("affiche les résultats des tests Prowlarr et qBittorrent", async ({ page }) => {
    await mockSettingsWrites(page);
    await page.route("**/api/prowlarr/test", async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ success: true, message: "Prowlarr reachable (e2e)" }),
      });
    });
    await page.route("**/api/qbittorrent/test", async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ success: true, message: "qBittorrent reachable (e2e)" }),
      });
    });

    await page.goto("/settings?tab=downloadTools");

    const prowlarr = settingsCard(page, "Prowlarr");
    await expect(prowlarr).toBeVisible();
    // Fill the connection fields before enabling: enabling an integration with
    // missing credentials is rejected by the shared validation policy.
    await prowlarr.getByPlaceholder("http://localhost:9696").fill("http://prowlarr.e2e:9696");
    await prowlarr.getByPlaceholder("Prowlarr API key").fill("e2e-prowlarr-key");
    await ensureEnabled(prowlarr);
    const prowlarrTest = prowlarr.getByRole("button", { name: "Test connection", exact: true });
    await expect(prowlarrTest).toBeEnabled();
    await prowlarrTest.click();
    await expect(prowlarr.getByText("Prowlarr reachable (e2e)", { exact: true })).toBeVisible();

    const qbittorrent = settingsCard(page, "qBittorrent");
    await expect(qbittorrent).toBeVisible();
    await qbittorrent.getByPlaceholder("http://localhost:8080").fill("http://qbittorrent.e2e:8080");
    await qbittorrent.locator('input[type="text"]').fill("e2e-qbittorrent");
    await ensureEnabled(qbittorrent);
    const qbTest = qbittorrent.getByRole("button", { name: "Test connection", exact: true });
    await expect(qbTest).toBeEnabled();
    await qbTest.click();
    await expect(qbittorrent.getByText("qBittorrent reachable (e2e)", { exact: true })).toBeVisible();
  });

  test("affiche le résultat du test Telegram", async ({ page }) => {
    await mockSettingsWrites(page);
    await page.route("**/api/telegram/test", async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ success: true, message: "Telegram bot reachable (e2e)" }),
      });
    });

    await page.goto("/settings?tab=notifications");

    const telegram = settingsCard(page, "Telegram");
    await expect(telegram).toBeVisible();
    // Fill the credentials before enabling: the shared validation policy keeps
    // a disabled bot from being enabled without a token and a chat id.
    await telegram.locator("#telegram-bot-token").fill("123456:e2e-bot-token");
    await telegram.locator("#telegram-chat-id").fill("987654321");
    await ensureEnabled(telegram);
    const testButton = telegram.getByRole("button", { name: "Test connection", exact: true });
    await expect(testButton).toBeEnabled();
    await testButton.click();

    await expect(telegram.getByText("Telegram bot reachable (e2e)", { exact: true })).toBeVisible();
  });

  test("affiche le résultat du test d'un fournisseur DDL", async ({ page }) => {
    await mockSettingsWrites(page);
    await page.route(/\/api\/ddl-providers\/[^/]+\/test$/, async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ success: true, message: "DDL provider reachable (e2e)" }),
      });
    });

    await page.goto("/settings?tab=downloadTools");

    const card = settingsCard(page, "Direct download");
    await expect(card).toBeVisible();
    const provider = card
      .locator("li")
      .filter({ has: page.getByRole("button", { name: "Test", exact: true }) })
      .first();
    test.skip((await provider.count()) === 0, "Aucun fournisseur DDL n'est disponible");

    const [request] = await Promise.all([
      page.waitForRequest(
        (candidate) =>
          candidate.method() === "POST" &&
          /\/api\/ddl-providers\/[^/]+\/test$/.test(new URL(candidate.url()).pathname)
      ),
      provider.getByRole("button", { name: "Test", exact: true }).click(),
    ]);
    expect(request).toBeTruthy();
    await expect(card.getByText("DDL provider reachable (e2e)", { exact: true })).toBeVisible();
  });

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
