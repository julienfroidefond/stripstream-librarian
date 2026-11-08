import { test, expect } from "@playwright/test";

import { uniqueName, waitForHydration } from "../helpers";
import { apiRequest } from "../support/api";

type StatusMapping = {
  id: string;
  provider_status: string;
  mapped_status: string | null;
};

test.describe("paramètres — actions locales", () => {
  test("vide le cache d'images et affiche le message de résultat", async ({ page }) => {
    await page.route("**/api/settings/cache/clear", (route) =>
      route.fulfill({ json: { success: true, message: "Cache E2E vidé" } })
    );
    await page.route("**/api/settings/cache/stats", (route) =>
      route.fulfill({
        json: {
          total_size_mb: 1,
          file_count: 2,
          directory: "/tmp/e2e-cache",
          memory_size_mb: 1,
          memory_page_count: 3,
          memory_max_size_mb: 128,
        },
      })
    );

    await page.goto("/settings?tab=media");
    await waitForHydration(page, "input");
    const clearResponse = page.waitForResponse(
      (response) =>
        response.request().method() === "POST" &&
        new URL(response.url()).pathname === "/api/settings/cache/clear"
    );
    await page.getByRole("button", { name: "Clear cache", exact: true }).click();
    expect((await clearResponse).ok()).toBe(true);
    await expect(page.getByText("Cache E2E vidé", { exact: true })).toBeVisible();
  });

  test("édite un format de renommage, met à jour l'aperçu et sauvegarde", async ({ page }) => {
    await page.route("**/api/settings/rename_format", (route) =>
      route.fulfill({ json: { success: true } })
    );

    await page.goto("/settings?tab=general");
    await waitForHydration(page, "input");
    const input = page.locator("#rename-format-regular");
    await expect(input).toBeVisible();

    await input.fill("Série {series_name} T{volume_padded} — {title}");
    await expect(
      page.getByText("Série Dragon Ball T01 — Son Goku et ses amis.cbz", { exact: true })
    ).toBeVisible();

    // Ajout d'une variable par clic dans la palette.
    await page.getByText("{authors}", { exact: true }).click();
    await expect(input).toHaveValue(/Série \{series_name\} T\{volume_padded\} — \{title\}\{authors\}$/);

    const saveRequest = page.waitForRequest(
      (request) =>
        request.method() === "POST" &&
        new URL(request.url()).pathname === "/api/settings/rename_format"
    );
    await page.locator("#rename-format-hs").click();
    const request = await saveRequest;
    expect(request.postDataJSON()).toEqual({
      value: "Série {series_name} T{volume_padded} — {title}{authors}",
    });
    await expect(page.getByText("Settings saved successfully", { exact: true })).toBeVisible();
  });

  test("teste le provider IA et restaure les prompts par défaut", async ({ page }) => {
    await page.route("**/api/settings/ai_tagging/test", (route) =>
      route.fulfill({ json: { message: "Mock AI provider reachable" } })
    );
    await page.route("**/api/settings/ai_tagging", (route) =>
      route.fulfill({ json: { success: true } })
    );

    await page.goto("/settings?tab=ai");
    await waitForHydration(page, "input");
    await page.getByRole("button", { name: "Test connection", exact: true }).click();
    await expect(page.getByText("Mock AI provider reachable", { exact: true })).toBeVisible();

    // `handleUpdateSetting` refuse de persister un objet contenant une valeur
    // vide : sans clé API, les prompts ne partiraient jamais.
    await page.locator("#ai-api-key").fill(uniqueName("sk-e2e"));
    const prompt = page.locator("#ai-prompt");
    const customPrompt = uniqueName("Prompt E2E");
    await prompt.fill(customPrompt);
    await expect(prompt).toHaveValue(customPrompt);
    const savePrompt = page.waitForRequest(
      (request) =>
        request.method() === "POST" &&
        new URL(request.url()).pathname === "/api/settings/ai_tagging"
    );
    await page.keyboard.press("Tab");
    expect((await savePrompt).postDataJSON().value.prompt).toBe(customPrompt);

    const restorePrompt = page.waitForRequest(
      (request) =>
        request.method() === "POST" &&
        new URL(request.url()).pathname === "/api/settings/ai_tagging"
    );
    await page.getByRole("button", { name: "Restore default prompt", exact: true }).click();
    expect((await restorePrompt).postDataJSON().value.prompt).toMatch(
      /^Suggest up to \{\{max_tags\}\}/
    );
    await expect(prompt).toHaveValue(/^Suggest up to \{\{max_tags\}\}/);

    const listsPrompt = page.locator("#ai-reading-lists-prompt");
    const customListsPrompt = uniqueName("Prompt listes E2E");
    await listsPrompt.fill(customListsPrompt);
    await expect(listsPrompt).toHaveValue(customListsPrompt);
    const saveListsPrompt = page.waitForRequest(
      (request) =>
        request.method() === "POST" &&
        new URL(request.url()).pathname === "/api/settings/ai_tagging"
    );
    await page.keyboard.press("Tab");
    await saveListsPrompt;

    const restoreListsPrompt = page.waitForRequest(
      (request) =>
        request.method() === "POST" &&
        new URL(request.url()).pathname === "/api/settings/ai_tagging"
    );
    await page
      .getByRole("button", { name: "Restore default reading list prompt", exact: true })
      .click();
    expect((await restoreListsPrompt).postDataJSON().value.reading_list_prompt).toMatch(
      /^You are a librarian organizing/
    );
    await expect(listsPrompt).toHaveValue(/^You are a librarian organizing/);
  });

  test("crée un statut cible, délie puis remappe un statut fournisseur", async ({ page }) => {
    const mappings = await apiRequest<StatusMapping[]>("/settings/status-mappings");
    const mapping = mappings.find((item) => item.mapped_status);
    test.skip(!mapping, "Aucun mapping fournisseur existant à manipuler");

    await page.route("**/api/settings/status-mappings/*", (route) => {
      if (route.request().method() !== "DELETE") {
        return route.continue();
      }
      return route.fulfill({
        json: {
          id: mapping!.id,
          provider_status: mapping!.provider_status,
          mapped_status: null,
        },
      });
    });
    await page.route("**/api/settings/status-mappings", (route) => {
      if (route.request().method() !== "POST") {
        return route.continue();
      }
      const body = route.request().postDataJSON() as {
        provider_status: string;
        mapped_status: string;
      };
      return route.fulfill({
        json: { id: mapping!.id, ...body },
      });
    });

    await page.goto("/settings?tab=metadata");
    await waitForHydration(page, "input");

    const target = uniqueName("e2e-target").toLocaleLowerCase();
    await page.getByPlaceholder("New target status (e.g. hiatus)").fill(target);
    await page.getByRole("button", { name: "Create status", exact: true }).click();
    await expect(page.getByText(target, { exact: true })).toBeVisible();

    const deleteRequest = page.waitForRequest(
      (request) =>
        request.method() === "DELETE" &&
        new URL(request.url()).pathname ===
          `/api/settings/status-mappings/${mapping!.id}`
    );
    await page
      .getByText(mapping!.provider_status, { exact: true })
      .locator("button")
      .click();
    await deleteRequest;

    // Le statut fournisseur apparaît maintenant dans la section « Unmapped ».
    const unmappedRow = page
      .getByText(mapping!.provider_status, { exact: true })
      .locator("xpath=..");
    const unmappedSelect = unmappedRow.locator("select");
    await expect(unmappedSelect).toBeVisible();
    const remapRequest = page.waitForRequest(
      (request) =>
        request.method() === "POST" &&
        new URL(request.url()).pathname === "/api/settings/status-mappings"
    );
    await unmappedSelect.selectOption(mapping!.mapped_status!);
    expect((await remapRequest).postDataJSON()).toEqual({
      provider_status: mapping!.provider_status,
      mapped_status: mapping!.mapped_status,
    });
    await expect(page.getByText("Unmapped", { exact: true })).toHaveCount(0);
  });

  test("teste, prévisualise, synchronise et importe AniList via des APIs mockées", async ({ page }) => {
    await page.route("**/api/settings/anilist", (route) =>
      route.fulfill({ json: { success: true } })
    );
    await page.route("**/api/anilist/status", (route) =>
      route.fulfill({
        json: {
          username: "E2E AniList User",
          user_id: 4242,
          site_url: "https://anilist.co/user/e2e",
        },
      })
    );
    await page.route("**/api/anilist/sync/preview", (route) =>
      route.fulfill({
        json: [
          {
            anilist_id: 101,
            anilist_title: "E2E Preview Anime",
            series_name: "E2E Preview Series",
            books_read: 1,
            book_count: 2,
            status: "CURRENT",
            anilist_url: "https://anilist.co/manga/101",
          },
        ],
      })
    );
    await page.route("**/api/anilist/sync", (route) =>
      route.fulfill({
        json: {
          synced: 2,
          skipped: 0,
          errors: [],
          items: [
            {
              series_name: "E2E Sync Series",
              anilist_title: "E2E Sync Anime",
              status: "COMPLETED",
              progress_volumes: 2,
              anilist_url: "https://anilist.co/manga/102",
            },
          ],
        },
      })
    );
    await page.route("**/api/anilist/pull", (route) =>
      route.fulfill({
        json: {
          updated: 1,
          skipped: 0,
          errors: [],
          items: [
            {
              series_name: "E2E Pull Series",
              anilist_title: "E2E Pull Anime",
              anilist_status: "CURRENT",
              books_updated: 2,
              anilist_url: "https://anilist.co/manga/103",
            },
          ],
        },
      })
    );

    await page.goto("/settings?tab=readingStatus");
    await waitForHydration(page, "input");
    await page.getByText("Manual token (advanced)", { exact: true }).click();
    await page.locator("#anilist-token").fill("e2e-anilist-token");

    const statusResponse = page.waitForResponse(
      (response) =>
        response.request().method() === "GET" &&
        new URL(response.url()).pathname === "/api/anilist/status"
    );
    await page.getByRole("button", { name: "Test connection", exact: true }).click();
    expect((await statusResponse).ok()).toBe(true);
    await expect(page.getByText("E2E AniList User", { exact: true })).toBeVisible();

    const previewResponse = page.waitForResponse(
      (response) =>
        response.request().method() === "GET" &&
        new URL(response.url()).pathname === "/api/anilist/sync/preview"
    );
    await page.getByRole("button", { name: "Preview", exact: true }).click();
    await previewResponse;
    await expect(page.getByText("1 series to sync", { exact: true })).toBeVisible();
    await expect(page.getByText("E2E Preview Anime", { exact: true })).toBeVisible();

    const syncResponse = page.waitForResponse(
      (response) =>
        response.request().method() === "POST" &&
        new URL(response.url()).pathname === "/api/anilist/sync"
    );
    await page.getByRole("button", { name: "Sync to AniList", exact: true }).click();
    await syncResponse;
    await expect(page.getByText("2 series synced", { exact: true })).toBeVisible();
    await expect(page.getByText("E2E Sync Anime", { exact: true })).toBeVisible();

    const pullResponse = page.waitForResponse(
      (response) =>
        response.request().method() === "POST" &&
        new URL(response.url()).pathname === "/api/anilist/pull"
    );
    await page.getByRole("button", { name: "Pull from AniList", exact: true }).click();
    await pullResponse;
    await expect(page.getByText("1 series updated", { exact: true })).toBeVisible();
    await expect(page.getByText("E2E Pull Anime", { exact: true })).toBeVisible();
  });

  test("synchronise Komga, ouvre les rapports et consulte l'historique", async ({ page }) => {
    const reportSummary = {
      id: "e2e-report-1",
      created_at: "2026-01-02T03:04:05Z",
      komga_url: "https://komga.e2e.local",
      total_komga_read: 5,
      matched: 1,
      already_read: 0,
      newly_marked: 1,
      unmatched_count: 1,
    };
    const syncResult = {
      id: "e2e-sync-1",
      created_at: "2026-02-03T04:05:06Z",
      komga_url: "https://komga.e2e.local",
      total_komga_read: 10,
      matched: 2,
      already_read: 1,
      newly_marked: 1,
      unmatched: ["Unknown E2E"],
      matched_books: ["Known E2E"],
      newly_marked_books: ["Known E2E"],
    };
    const report = {
      ...syncResult,
      id: "e2e-report-1",
      total_komga_read: 5,
      matched: 1,
      already_read: 1,
      newly_marked: 0,
      unmatched: ["Report Unknown E2E"],
      matched_books: ["Report E2E"],
      newly_marked_books: [],
    };

    await page.route("**/api/komga/reports/e2e-report-1", (route) => route.fulfill({ json: report }));
    await page.route("**/api/komga/reports", (route) => route.fulfill({ json: [reportSummary] }));
    await page.route("**/api/komga/sync", (route) => route.fulfill({ json: syncResult }));
    await page.route("**/api/settings/komga", (route) => route.fulfill({ json: { success: true } }));

    await page.goto("/settings?tab=readingStatus");
    await waitForHydration(page, "input");
    const komga = page
      .locator("section")
      .filter({ has: page.getByRole("heading", { name: "Komga sync" }) });
    const inputs = komga.locator("input");
    await komga.locator('input[type="url"]').fill("https://komga.e2e.local");
    await inputs.nth(1).fill("e2e-komga-user");
    await inputs.nth(2).fill("e2e-komga-password");

    const syncResponse = page.waitForResponse(
      (response) =>
        response.request().method() === "POST" &&
        new URL(response.url()).pathname === "/api/komga/sync"
    );
    await komga.getByRole("button", { name: "Sync read books", exact: true }).click();
    expect((await syncResponse).ok()).toBe(true);

    await expect(komga.getByText("Read on Komga", { exact: true })).toBeVisible();
    await komga.getByRole("button", { name: "1 matched book", exact: true }).click();
    await expect(komga.getByText("Known E2E", { exact: true })).toBeVisible();
    await komga.getByRole("button", { name: "1 unmatched book", exact: true }).click();
    await expect(komga.getByText("Unknown E2E", { exact: true })).toBeVisible();

    const reportResponse = page.waitForResponse(
      (response) =>
        response.request().method() === "GET" &&
        new URL(response.url()).pathname === "/api/komga/reports/e2e-report-1"
    );
    await komga.getByRole("button", { name: /komga\.e2e\.local/ }).click();
    await reportResponse;

    // Attendre le rendu du rapport avant de viser le dernier bouton « matched » :
    // sinon le clic peut encore viser celui du résultat de synchronisation.
    await expect(komga.getByText("Read on Komga", { exact: true })).toHaveCount(2);
    await komga
      .getByRole("button", { name: "1 matched book", exact: true })
      .last()
      .click();
    await expect(komga.getByText("Report E2E", { exact: true })).toBeVisible();
  });

  test("configure, connecte et déconnecte Telegram, puis gère les chaînes", async ({ page }) => {
    let authorized = false;
    let configured = false;
    let syncInterval = 30;
    const sources: Array<{ id: string; channel_username: string; channel_title: string | null }> = [];

    await page.route("**/api/telegram-monitor/status", (route) =>
      route.fulfill({
        json: {
          authorized,
          configured,
          phone: "+33600000000",
          api_id: 123456,
          sync_interval_minutes: syncInterval,
        },
      })
    );
    await page.route("**/api/telegram-monitor/sources**", (route) => {
      const method = route.request().method();
      const path = new URL(route.request().url()).pathname;
      if (method === "GET" && path.endsWith("/sources")) {
        return route.fulfill({ json: sources });
      }
      if (method === "POST" && path.endsWith("/sources")) {
        const body = route.request().postDataJSON() as { channel_username: string };
        const source = {
          id: "e2e-source-1",
          channel_username: body.channel_username,
          channel_title: "E2E Channel",
        };
        sources.push(source);
        return route.fulfill({ json: source });
      }
      if (method === "DELETE" && path.endsWith("/sources/e2e-source-1")) {
        sources.splice(0, sources.length);
        return route.fulfill({ json: {} });
      }
      return route.continue();
    });
    await page.route("**/api/telegram-monitor/settings", (route) => {
      configured = true;
      const body = route.request().postDataJSON() as {
        sync_interval_minutes?: number;
      };
      if (body.sync_interval_minutes) syncInterval = body.sync_interval_minutes;
      return route.fulfill({ json: { success: true } });
    });
    await page.route("**/api/telegram-monitor/auth/start", (route) =>
      route.fulfill({ json: { success: true } })
    );
    await page.route("**/api/telegram-monitor/auth/verify", (route) => {
      authorized = true;
      return route.fulfill({ json: { success: true } });
    });
    await page.route("**/api/telegram-monitor/auth", (route) => {
      authorized = false;
      return route.fulfill({ json: { success: true } });
    });
    await page.route("**/api/telegram-monitor/channels*", (route) => route.fulfill({ json: [] }));

    await page.goto("/settings?tab=downloadTools");
    await waitForHydration(page, "input");
    const telegram = page
      .locator("section")
      .filter({ has: page.getByRole("heading", { name: "Telegram — Channels" }) });
    await expect(telegram.getByText("Not authenticated", { exact: true })).toBeVisible();

    await page.locator("#tg-api-id").fill("123456");
    await page.locator("#tg-api-hash").fill("e2e-api-hash");
    await page.locator("#tg-phone").fill("+33600000000");
    const saveSettings = page.waitForRequest(
      (request) =>
        request.method() === "POST" &&
        new URL(request.url()).pathname === "/api/telegram-monitor/settings"
    );
    await telegram.getByRole("button", { name: "Save", exact: true }).click();
    await saveSettings;
    const sendCode = telegram.getByRole("button", { name: "Send code", exact: true });
    await expect(sendCode).toBeVisible();

    await sendCode.click();
    await expect(page.getByText("Code sent to your phone/Telegram app", { exact: true })).toBeVisible();
    await page.locator("#tg-code").fill("12345");
    const verifyResponse = page.waitForResponse(
      (response) =>
        response.request().method() === "POST" &&
        new URL(response.url()).pathname === "/api/telegram-monitor/auth/verify"
    );
    await telegram.getByRole("button", { name: "Verify", exact: true }).click();
    await verifyResponse;
    await expect(telegram.getByRole("heading", { name: "Monitored channels" })).toBeVisible();

    await page.locator("#tg-new-channel").fill("e2e_channel");
    const addSourceResponse = page.waitForResponse(
      (response) =>
        response.request().method() === "POST" &&
        new URL(response.url()).pathname === "/api/telegram-monitor/sources"
    );
    await telegram.getByRole("button", { name: "Add", exact: true }).click();
    await addSourceResponse;
    await expect(telegram.getByText("@e2e_channel", { exact: true })).toBeVisible();

    const deleteSource = page.waitForResponse(
      (response) =>
        response.request().method() === "DELETE" &&
        new URL(response.url()).pathname === "/api/telegram-monitor/sources/e2e-source-1"
    );
    const sourceRow = telegram
      .getByText("@e2e_channel", { exact: true })
      .locator("xpath=..");
    await sourceRow.getByRole("button", { name: "×", exact: true }).click();
    await deleteSource;
    await expect(telegram.getByText("@e2e_channel", { exact: true })).toHaveCount(0);

    const intervalSelect = telegram.locator("select").first();
    const intervalRequest = page.waitForRequest(
      (request) =>
        request.method() === "POST" &&
        new URL(request.url()).pathname === "/api/telegram-monitor/settings" &&
        (request.postDataJSON() as { sync_interval_minutes?: number })
          .sync_interval_minutes === 60
    );
    await intervalSelect.selectOption("60");
    await intervalRequest;

    // Les identifiants sont repliés une fois le compte connecté.
    await telegram.getByRole("button", { name: /API credentials/ }).click();
    await telegram.getByRole("button", { name: "Disconnect", exact: true }).click();
    await expect(telegram.getByText("Not authenticated", { exact: true })).toBeVisible();
  });
});
