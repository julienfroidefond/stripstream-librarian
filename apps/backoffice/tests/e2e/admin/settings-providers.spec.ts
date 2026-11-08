import { test, expect, type Page } from "@playwright/test";

import { uniqueName, waitForHydration } from "../helpers";

function exactPath(request: { url(): string }, path: string) {
  return new URL(request.url()).pathname === path;
}

async function saveMetadataProviders(page: Page, action: () => Promise<void>) {
  const requestPromise = page.waitForRequest(
    (request) =>
      request.method() === "POST" &&
      exactPath(request, "/api/settings/metadata_providers")
  );
  await action();
  return (await requestPromise).postDataJSON() as {
    value: Record<string, unknown>;
  };
}

async function saveLimits(page: Page, action: () => Promise<void>) {
  const requestPromise = page.waitForRequest(
    (request) =>
      request.method() === "POST" && exactPath(request, "/api/settings/limits")
  );
  await action();
  return (await requestPromise).postDataJSON() as {
    value: Record<string, unknown>;
  };
}

test.describe("paramètres — providers", () => {
  test("choisit le provider de métadonnées, la langue et les clés sans persister", async ({
    page,
  }) => {
    await page.route("**/api/metadata/providers", (route) =>
      route.fulfill({
        json: [
          {
            id: "google_books",
            label: "Google Books",
            requires_api_key: false,
          },
          {
            id: "open_library",
            label: "Open Library",
            requires_api_key: false,
          },
          {
            id: "comicvine",
            label: "ComicVine",
            requires_api_key: true,
          },
        ],
      })
    );
    await page.route("**/api/settings/metadata_providers", (route) =>
      route.fulfill({ json: { success: true } })
    );

    await page.goto("/settings?tab=metadata");
    await waitForHydration(page, "input");

    const card = page
      .locator("section")
      .filter({ has: page.getByRole("heading", { name: "Metadata providers" }) });
    await expect(card.getByRole("button", { name: "Open Library" })).toBeVisible();

    const providerSave = await saveMetadataProviders(page, () =>
      card.getByRole("button", { name: "Open Library", exact: true }).click()
    );
    expect(providerSave.value).toMatchObject({
      default_provider: "open_library",
      metadata_language: "en",
    });

    const languageSave = await saveMetadataProviders(page, () =>
      card.getByRole("button", { name: "Français", exact: true }).click()
    );
    expect(languageSave.value).toMatchObject({
      default_provider: "open_library",
      metadata_language: "fr",
    });

    const apiKey = uniqueName("e2e-google-key");
    const keySave = await saveMetadataProviders(page, async () => {
      await card.locator("#metadata-google-books-key").fill(apiKey);
      await card.locator("#metadata-google-books-key").blur();
    });
    expect(keySave.value).toMatchObject({
      default_provider: "open_library",
      metadata_language: "fr",
      google_books: { api_key: apiKey },
    });
    await expect(
      page.getByText("Settings saved successfully", { exact: true }).first()
    ).toBeVisible();
  });

  test("borne les limites du moteur DDL à chaque perte de focus", async ({ page }) => {
    await page.route("**/api/settings/limits", (route) =>
      route.fulfill({ json: { success: true } })
    );

    await page.goto("/settings?tab=downloadTools");
    await waitForHydration(page, "input");

    const fields = [
      ["concurrent_ddl_provider_downloads", 1],
      ["ddl_download_connections", 1],
      ["download_max_attempts", 1],
      ["download_stall_timeout_seconds", 10],
      ["download_timeout_minutes", 1],
    ] as const;

    for (const [key, expectedMinimum] of fields) {
      const input = page.locator(`#ddl-engine-${key}`);
      await input.fill("0");
      const save = await saveLimits(page, () => input.blur());
      expect(save.value[key]).toBe(expectedMinimum);
      await expect(input).toHaveValue(String(expectedMinimum));
    }

    const maximumSave = await saveLimits(page, async () => {
      await page.locator("#ddl-engine-concurrent_ddl_provider_downloads").fill("999");
      await page.locator("#ddl-engine-concurrent_ddl_provider_downloads").blur();
    });
    expect(maximumSave.value.concurrent_ddl_provider_downloads).toBe(16);
    await expect(page.locator("#ddl-engine-concurrent_ddl_provider_downloads")).toHaveValue("16");
  });

  test("active, teste et recherche avec un provider DDL mocké", async ({ page }) => {
    let enabled = false;
    let baseUrl = "https://ddl.e2e";
    let searchInterval = 0;
    const providers = () => [
      {
        id: "e2e-ddl",
        label: "E2E Direct",
        kind: "json",
        description: "Provider DDL de test",
        base_url: baseUrl,
        requires_auth: false,
        enabled,
      },
    ];

    await page.route("**/api/ddl-providers**", (route) => {
      const request = route.request();
      const path = new URL(request.url()).pathname;

      if (request.method() === "GET" && path === "/api/ddl-providers") {
        return route.fulfill({ json: providers() });
      }
      if (request.method() === "GET" && path === "/api/ddl-providers/search-settings") {
        return route.fulfill({ json: { search_interval_minutes: searchInterval } });
      }
      if (request.method() === "POST" && path === "/api/ddl-providers/search-settings") {
        const body = request.postDataJSON() as { search_interval_minutes: number };
        searchInterval = body.search_interval_minutes;
        return route.fulfill({ json: { search_interval_minutes: searchInterval } });
      }
      if (request.method() === "POST" && path === "/api/ddl-providers/e2e-ddl/toggle") {
        const body = request.postDataJSON() as {
          enabled: boolean;
          base_url?: string;
        };
        enabled = body.enabled;
        if (body.base_url) baseUrl = body.base_url;
        return route.fulfill({ json: providers() });
      }
      if (request.method() === "POST" && path === "/api/ddl-providers/e2e-ddl/test") {
        return route.fulfill({
          json: {
            success: true,
            message: "E2E provider reachable",
            result_count: 3,
          },
        });
      }
      if (request.method() === "POST" && path === "/api/ddl-providers/search-all") {
        return route.fulfill({ json: { status: "queued" } });
      }
      return route.continue();
    });

    await page.goto("/settings?tab=downloadTools");
    await waitForHydration(page, "input");

    const card = page
      .locator("section")
      .filter({ has: page.getByRole("heading", { name: "Direct download", exact: true }) });
    await expect(card.getByText("E2E Direct", { exact: true })).toBeVisible();

    const searchNow = card.getByRole("button", { name: "Search now", exact: true });
    await expect(searchNow).toBeDisabled();
    await expect(searchNow).toHaveAttribute("title", "Enable a provider first");

    const toggle = card.getByRole("checkbox", { name: "E2E Direct", exact: true });
    const toggleRequest = page.waitForRequest(
      (request) =>
        request.method() === "POST" &&
        exactPath(request, "/api/ddl-providers/e2e-ddl/toggle")
    );
    await toggle.locator("xpath=..").click();
    expect((await toggleRequest).postDataJSON()).toEqual({
      enabled: true,
      base_url: "https://ddl.e2e",
    });
    await expect(searchNow).toBeEnabled();
    await expect(page.getByText("Provider updated", { exact: true }).first()).toBeVisible();

    const baseUrlSave = page.waitForRequest(
      (request) =>
        request.method() === "POST" &&
        exactPath(request, "/api/ddl-providers/e2e-ddl/toggle")
    );
    const baseUrlInput = card.getByRole("textbox", { name: "Base URL", exact: true });
    await baseUrlInput.fill("https://override.e2e");
    await baseUrlInput.blur();
    expect((await baseUrlSave).postDataJSON()).toEqual({
      enabled: true,
      base_url: "https://override.e2e",
    });

    const testResponse = page.waitForResponse(
      (response) =>
        response.request().method() === "POST" &&
        exactPath(response.request(), "/api/ddl-providers/e2e-ddl/test")
    );
    await card.getByRole("button", { name: "Test", exact: true }).click();
    expect((await testResponse).ok()).toBe(true);
    await expect(card.getByText("E2E provider reachable", { exact: true })).toBeVisible();

    const intervalSave = page.waitForRequest(
      (request) =>
        request.method() === "POST" &&
        exactPath(request, "/api/ddl-providers/search-settings")
    );
    await card.getByRole("combobox", { name: "Background search cadence" }).selectOption("60");
    expect((await intervalSave).postDataJSON()).toEqual({ search_interval_minutes: 60 });

    const searchResponse = page.waitForResponse(
      (response) =>
        response.request().method() === "POST" &&
        exactPath(response.request(), "/api/ddl-providers/search-all")
    );
    await searchNow.click();
    expect((await searchResponse).ok()).toBe(true);
    await expect(
      page.getByText("Search started in the background", { exact: true }).first()
    ).toBeVisible();
  });
});
