import { test, expect, type Page, type Route } from "@playwright/test";

import { uniqueName } from "../helpers";
import { apiRequest, deleteSeries } from "../support/api";
import { METADATA_LIBRARY_NAME } from "../support/metadata";

type Suggestion = {
  provider: string;
  external_id: string;
  title: string;
  authors: string[];
  description: string | null;
  genres: string[];
  cover_url: string | null;
  external_url: string | null;
  start_year: number | null;
  total_volumes: number | null;
  status: string | null;
  rating: number | null;
};

type ProwlarrItem = {
  series_name: string;
  release_count: number;
  best_seeders: number;
  total_seeders: number;
  categories: string[];
  indexers: string[];
  best_release_title: string;
  best_download_url: string | null;
  best_size: number;
  best_publish_date: string | null;
  best_info_url: string | null;
  best_indexer: string | null;
  volumes_found: number[];
  local_series_id: string | null;
  local_series_name: string | null;
  volumes_already_owned: number[];
};

type SeriesPage = { items: Array<{ series_id: string; name: string }> };

function makeSuggestion(title: string, externalId: string): Suggestion {
  return {
    provider: "senscritique",
    external_id: externalId,
    title,
    authors: ["E2E Discovery Author"],
    description: "Fixture découverte",
    genres: ["E2E Discovery Genre"],
    cover_url: null,
    external_url: null,
    start_year: 2026,
    total_volumes: 1,
    status: "RELEASING",
    rating: 8.4,
  };
}

async function mockDiscoveryBrowserApis(page: Page, suggestion: Suggestion): Promise<void> {
  await page.route("**/api/discovery/trending*", (route) =>
    route.fulfill({ json: [suggestion] })
  );
  await page.route("**/api/discovery/hide", (route) => route.fulfill({ json: {} }));
  await page.route("**/api/discovery/unhide", (route) => route.fulfill({ json: {} }));
}

async function chooseDiscoverySuggestion(page: Page, title: string): Promise<void> {
  await page.getByRole("button", { name: "Nouveautés Manga", exact: true }).click();
  const card = page.locator(".group").filter({ has: page.getByRole("heading", { name: title }) });
  await expect(card).toBeVisible();
}

function prowlarrFixtures(): ProwlarrItem[] {
  return Array.from({ length: 30 }, (_, index) => {
    const indexer = index < 20 ? "IndexerA" : "IndexerB";
    return {
      series_name: `E2E Prowlarr ${index + 1}`,
      release_count: 1,
      best_seeders: index + 1,
      total_seeders: index + 1,
      categories: ["Comics (7030)"],
      indexers: [indexer],
      best_release_title: `E2E Release ${String(index + 1).padStart(2, "0")}`,
      best_download_url: null,
      best_size: 1024 * 1024,
      best_publish_date: new Date(Date.UTC(2026, 0, index + 1)).toISOString(),
      best_info_url: null,
      best_indexer: indexer,
      volumes_found: [index + 1],
      local_series_id: index === 25 ? "00000000-0000-0000-0000-000000000025" : null,
      local_series_name: index === 25 ? "E2E Owned" : null,
      volumes_already_owned: index === 25 ? [26] : [],
    };
  });
}

async function fulfillProwlarr(route: Route, items: ProwlarrItem[]): Promise<void> {
  const params = new URL(route.request().url()).searchParams;
  const indexer = params.get("indexer");
  const sort = params.get("sort");
  let data = indexer ? items.filter((item) => item.best_indexer === indexer) : items;
  if (sort === "date") {
    data = [...data].sort((a, b) =>
      (b.best_publish_date ?? "").localeCompare(a.best_publish_date ?? "")
    );
  }
  await route.fulfill({
    json: { items: data, all_indexers: ["IndexerA", "IndexerB"] },
  });
}

test.describe("découverte — actions des onglets et filtres", () => {
  test("bascule provider/période, hard refresh, masque et réaffiche une série", async ({ page }) => {
    const title = uniqueName("E2E Discovery Action");
    const suggestion = makeSuggestion(title, uniqueName("e2e-discovery"));
    const hiddenTitle = uniqueName("E2E Hidden Discovery");

    await mockDiscoveryBrowserApis(page, suggestion);
    await page.route("**/api/discovery/hidden", (route) =>
      route.fulfill({
        json: [
          {
            id: "e2e-hidden-1",
            provider: suggestion.provider,
            external_id: suggestion.external_id,
            title: hiddenTitle,
            cover_url: null,
          },
        ],
      })
    );

    await page.goto("/discovery");
    await expect(page.locator("main h1")).toContainText("Series Discovery");
    await chooseDiscoverySuggestion(page, title);

    // Filtre de période : nouvelle requête puis suggestions toujours affichées.
    const periodRequest = page.waitForRequest((request) => {
      if (!request.url().includes("/api/discovery/trending")) return false;
      return new URL(request.url()).searchParams.get("period") === "year";
    });
    await page.getByRole("button", { name: "Année", exact: true }).click();
    await periodRequest;
    await expect(page.getByRole("heading", { name: title })).toBeVisible();

    // Hard refresh : contourne le cache côté API.
    const refreshRequest = page.waitForRequest((request) => {
      if (!request.url().includes("/api/discovery/trending")) return false;
      return new URL(request.url()).searchParams.get("nocache") === "true";
    });
    await page.getByTitle("Refresh (bypass cache)").click();
    await refreshRequest;
    await expect(page.getByRole("heading", { name: title })).toBeVisible();

    // Masquage : la carte disparaît immédiatement.
    const card = page.locator(".group").filter({ has: page.getByRole("heading", { name: title }) });
    await card.getByTitle("Hide", { exact: true }).click();
    await expect(card).toHaveCount(0);

    // Panneau des masquées : réaffichage via l'API mockée.
    await page.locator('button[title="Hidden series"]').click();
    await expect(page.getByRole("heading", { name: "Hidden series" })).toBeVisible();
    await expect(page.getByText(hiddenTitle, { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Unhide", exact: true }).click();
    await expect(page.getByText(hiddenTitle, { exact: true })).toHaveCount(0);
    await expect(page.getByText("No hidden series", { exact: true })).toBeVisible();
  });

  test("ajoute une suggestion à une bibliothèque via la modale", async ({ page }) => {
    test.setTimeout(90_000);

    const title = uniqueName("E2E Discovery Add");
    const suggestion = makeSuggestion(title, uniqueName("e2e-discovery-add"));
    let createdSeriesId: string | null = null;

    await mockDiscoveryBrowserApis(page, suggestion);
    await page.route("**/api/metadata/providers", (route) => route.fulfill({ json: [] }));
    await page.route("**/api/metadata/search", (route) => route.fulfill({ json: [] }));
    await page.route("**/api/series?*", (route) => route.fulfill({ json: { items: [] } }));

    try {
      await page.goto("/discovery");
      await chooseDiscoverySuggestion(page, title);

      const card = page.locator(".group").filter({ has: page.getByRole("heading", { name: title }) });
      await card.getByRole("button", { name: "Add", exact: true }).click();

      const panel = page.getByTestId("modal-panel");
      await expect(panel.getByText("Select a library", { exact: true }).first()).toBeVisible();
      await panel.getByRole("button", { name: METADATA_LIBRARY_NAME, exact: true }).click();

      const addButton = panel.getByRole("button", { name: "Add to library", exact: true });
      await expect(addButton).toBeEnabled();
      await addButton.click();

      // La carte (et sa modale, rendue dans la carte) est démontée dès l'ajout :
      // la disparition de la carte prouve que l'écriture a réussi.
      await expect(card).toHaveCount(0, { timeout: 15_000 });

      let found: SeriesPage["items"][number] | undefined;
      await expect
        .poll(async () => {
          const result = await apiRequest<SeriesPage>(
            `/series?q=${encodeURIComponent(title)}&limit=10`
          );
          found = result.items.find((item) => item.name === title);
          return Boolean(found);
        })
        .toBe(true);
      createdSeriesId = found!.series_id;
    } finally {
      if (createdSeriesId) await deleteSeries(createdSeriesId).catch(() => undefined);
    }
  });

  test("filtres, tri, recherche et pagination Prowlarr", async ({ page }) => {
    test.setTimeout(90_000);
    const items = prowlarrFixtures();
    await page.route("**/api/discovery/prowlarr*", (route) => fulfillProwlarr(route, items));

    await page.goto("/discovery");
    const prowlarrTab = page.getByRole("button", { name: "Prowlarr", exact: true });
    test.skip(
      (await prowlarrTab.count()) === 0,
      "Onglet Prowlarr absent : flag downloads_enabled désactivé"
    );

    await prowlarrTab.click();
    const rows = page.locator("main table tbody tr");
    await expect(rows).toHaveCount(25);
    await expect(page.getByText("1 / 2", { exact: true })).toBeVisible();

    await page.getByRole("button", { name: "›", exact: true }).click();
    await expect(rows).toHaveCount(5);
    await expect(page.getByText("2 / 2", { exact: true })).toBeVisible();

    // Tri par date : la requête est bien transmise à l'API.
    const sortRequest = page.waitForRequest((request) => {
      if (!request.url().includes("/api/discovery/prowlarr")) return false;
      return new URL(request.url()).searchParams.get("sort") === "date";
    });
    await page.getByRole("button", { name: "Recent", exact: true }).click();
    await sortRequest;
    await expect(rows).toHaveCount(25);

    // Filtre provider : seuls les items de l'indexeur choisi restent.
    const indexerRequest = page.waitForRequest((request) => {
      if (!request.url().includes("/api/discovery/prowlarr")) return false;
      return new URL(request.url()).searchParams.get("indexer") === "IndexerB";
    });
    await page.getByRole("button", { name: "IndexerB", exact: true }).click();
    await indexerRequest;
    await expect(rows).toHaveCount(10);

    // Masque les séries déjà possédées.
    await page.getByRole("button", { name: "Hide owned", exact: true }).click();
    await expect(rows).toHaveCount(9);

    // Recherche locale sur le titre de release.
    await page.getByPlaceholder("Search", { exact: true }).fill("E2E Release 30");
    await expect(rows).toHaveCount(1);
    await expect(rows.first()).toContainText("E2E Release 30");

    // Filtre catégorie : nouvelle requête scopée.
    const categoryRequest = page.waitForRequest((request) => {
      if (!request.url().includes("/api/discovery/prowlarr")) return false;
      return new URL(request.url()).searchParams.get("category") === "7030";
    });
    await page.getByRole("button", { name: /^Comics/ }).click();
    await categoryRequest;
  });
});
