import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const cookieStore = vi.hoisted(() => ({ get: vi.fn() }));

vi.mock("next/headers", () => ({
  cookies: async () => cookieStore,
}));

import {
  createLibrary,
  createToken,
  deleteLibrary,
  deleteSeriesRating,
  fetchAllSeries,
  fetchAuthors,
  fetchBooks,
  fetchDownloadsEnabled,
  fetchGapSummary,
  fetchLibraries,
  fetchRelatedSeries,
  fetchSeriesById,
  fetchStats,
  listFolders,
  markSeriesRead,
  mergeSeries,
  rebuildIndex,
  scanLibrary,
  searchBooks,
  syncKomga,
  updateBook,
  updateLibraryMonitoring,
  updateReadingProgress,
  updateSeriesRating,
  updateSetting,
  updateToken,
} from "@/lib/api";

function makeResponse(body: unknown, status = 200): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    text: async () => (typeof body === "string" ? body : JSON.stringify(body)),
    json: async () => body,
  } as unknown as Response;
}

let fetchMock: ReturnType<typeof vi.fn>;

beforeEach(() => {
  process.env.API_BASE_URL = "http://api:7080";
  process.env.API_BOOTSTRAP_TOKEN = "bootstrap-token";
  cookieStore.get.mockReset();
  cookieStore.get.mockReturnValue(undefined);
  fetchMock = vi.fn().mockResolvedValue(makeResponse({ ok: true }));
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

type Call = {
  url: string;
  method: string;
  body: unknown;
  init: RequestInit & { next?: unknown };
};

function lastCall(): Call {
  const calls = fetchMock.mock.calls;
  const [url, init] = calls[calls.length - 1] as [string, RequestInit];
  return {
    url,
    method: init.method ?? "GET",
    body: init.body ? JSON.parse(init.body as string) : undefined,
    init,
  };
}

describe("libraries", () => {
  it("lists libraries with an ISR tag", async () => {
    await fetchLibraries();
    expect(lastCall()).toMatchObject({
      url: "http://api:7080/libraries",
      method: "GET",
    });
    expect(lastCall().init.next).toEqual({ revalidate: 30 });
  });

  it("creates a library", async () => {
    await createLibrary("Manga", "/libraries/manga");
    expect(lastCall()).toMatchObject({
      url: "http://api:7080/libraries",
      method: "POST",
      body: { name: "Manga", root_path: "/libraries/manga" },
    });
  });

  it("deletes a library", async () => {
    await deleteLibrary("lib-1");
    expect(lastCall()).toMatchObject({
      url: "http://api:7080/libraries/lib-1",
      method: "DELETE",
    });
  });

  it("scans a library with and without the full flag", async () => {
    await scanLibrary("lib-1");
    expect(lastCall()).toMatchObject({ url: "http://api:7080/libraries/lib-1/scan", body: {} });

    await scanLibrary("lib-1", true);
    expect(lastCall()).toMatchObject({ method: "POST", body: { full: true } });
  });

  it("patches monitoring with only the provided options", async () => {
    await updateLibraryMonitoring("lib-1", true, "auto");
    expect(lastCall()).toMatchObject({
      url: "http://api:7080/libraries/lib-1/monitoring",
      method: "PATCH",
      body: { monitor_enabled: true, scan_mode: "auto" },
    });

    await updateLibraryMonitoring("lib-1", false, "manual", true, "smart", "watcher");
    expect(lastCall().body).toEqual({
      monitor_enabled: false,
      scan_mode: "manual",
      watcher_enabled: true,
      metadata_refresh_mode: "smart",
      download_detection_mode: "watcher",
    });
  });
});

describe("index jobs", () => {
  it("rebuilds the index with conditional flags", async () => {
    await rebuildIndex();
    expect(lastCall()).toMatchObject({ url: "http://api:7080/index/rebuild", body: {} });

    await rebuildIndex("lib-1", true, true);
    expect(lastCall().body).toEqual({ library_id: "lib-1", full: true, rescan: true });
  });
});

describe("folders", () => {
  it("lists the root without a query", async () => {
    await listFolders();
    expect(lastCall().url).toBe("http://api:7080/folders");
  });

  it("encodes the requested path", async () => {
    await listFolders("/libraries/My Books");
    expect(lastCall().url).toBe(
      "http://api:7080/folders?path=%2Flibraries%2FMy%20Books"
    );
  });
});

describe("tokens", () => {
  it("creates a token without a user", async () => {
    await createToken("ci", "read");
    expect(lastCall()).toMatchObject({
      url: "http://api:7080/admin/tokens",
      method: "POST",
      body: { name: "ci", scope: "read" },
    });
  });

  it("creates a token bound to a user", async () => {
    await createToken("ci", "read", "user-1");
    expect(lastCall().body).toEqual({ name: "ci", scope: "read", user_id: "user-1" });
  });

  it("clears the token user with null", async () => {
    await updateToken("tok-1", null);
    expect(lastCall()).toMatchObject({
      url: "http://api:7080/admin/tokens/tok-1",
      method: "PATCH",
      body: { user_id: null },
    });
  });
});

describe("books", () => {
  it("builds the book list query from every filter", async () => {
    await fetchBooks("lib-1", "Berserk", 2, 24, "reading", "title", "Miura", "cbz", "anilist", "dragon");

    expect(lastCall().url).toBe(
      "http://api:7080/books?q=dragon&library_id=lib-1&series=Berserk&reading_status=reading&sort=title&author=Miura&format=cbz&metadata_provider=anilist&page=2&limit=24"
    );
    expect(lastCall().init.next).toEqual({ revalidate: 15, tags: ["books"] });
  });

  it("falls back to the default page and limit", async () => {
    await fetchBooks();
    expect(lastCall().url).toBe("http://api:7080/books?page=1&limit=50");
  });

  it("adds the metadata gap filter", async () => {
    await fetchBooks("lib-1", undefined, 1, 24, undefined, undefined, undefined, undefined, undefined, undefined, "no_isbn");
    expect(lastCall().url).toBe("http://api:7080/books?library_id=lib-1&gap=no_isbn&page=1&limit=24");
  });

  it("patches a book with the raw payload", async () => {
    const data = {
      title: "Berserk",
      author: null,
      authors: ["Miura"],
      series: "Berserk",
      volume: 1,
      language: "fr",
      summary: null,
      isbn: null,
      publish_date: null,
    };
    await updateBook("book-1", data);
    expect(lastCall()).toMatchObject({
      url: "http://api:7080/books/book-1",
      method: "PATCH",
      body: data,
    });
  });

  it("updates reading progress with a null page by default", async () => {
    await updateReadingProgress("book-1", "reading");
    expect(lastCall()).toMatchObject({
      url: "http://api:7080/books/book-1/progress",
      method: "PATCH",
      body: { status: "reading", current_page: null },
    });
  });

  it("sends the current page when provided", async () => {
    await updateReadingProgress("book-1", "read", 12);
    expect(lastCall().body).toEqual({ status: "read", current_page: 12 });
  });
});

describe("series", () => {
  it("builds the series query from the boolean flags", async () => {
    await fetchAllSeries(undefined, "Berserk", undefined, 1, 50, undefined, undefined, true, undefined, undefined, true, true, "volume", "true");

    expect(lastCall().url).toBe(
      "http://api:7080/series?q=Berserk&has_missing=true&no_books=true&has_books=true&volume_type=volume&rated_only=true&page=1&limit=50"
    );
    expect(lastCall().init.next).toEqual({ revalidate: 15, tags: ["series"] });
  });

  it("adds the metadata gap filter", async () => {
    await fetchAllSeries("lib-1", undefined, undefined, 1, 24, undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined, "no_description");
    expect(lastCall().url).toBe("http://api:7080/series?library_id=lib-1&gap=no_description&page=1&limit=24");
  });

  it("fetches a series detail", async () => {
    await fetchSeriesById("ser-1");
    expect(lastCall().url).toBe("http://api:7080/series/ser-1/details");
  });

  it("limits related series", async () => {
    await fetchRelatedSeries("ser-1", 5);
    expect(lastCall().url).toBe("http://api:7080/series/ser-1/related?limit=5");
  });

  it("merges a source series into a target", async () => {
    await mergeSeries("target", "source");
    expect(lastCall()).toMatchObject({
      url: "http://api:7080/series/target/merge",
      method: "POST",
      body: { source_id: "source" },
    });
  });

  it("marks a series read by default", async () => {
    await markSeriesRead("ser-1");
    expect(lastCall()).toMatchObject({
      url: "http://api:7080/series/mark-read",
      method: "POST",
      body: { series: "ser-1", status: "read" },
    });
  });

  it("rates and unrates a series", async () => {
    await updateSeriesRating("ser-1", 8);
    expect(lastCall()).toMatchObject({
      url: "http://api:7080/series/ser-1/rating",
      method: "PUT",
      body: { rating: 8 },
    });

    await deleteSeriesRating("ser-1");
    expect(lastCall()).toMatchObject({ method: "DELETE" });
  });
});

describe("search and stats", () => {
  it("searches books", async () => {
    await searchBooks("berserk");
    expect(lastCall().url).toBe("http://api:7080/search?q=berserk&limit=20");
  });

  it("omits the default stats period", async () => {
    await fetchStats();
    expect(lastCall().url).toBe("http://api:7080/stats");

    await fetchStats("week");
    expect(lastCall().url).toBe("http://api:7080/stats");

    await fetchStats("month");
    expect(lastCall().url).toBe("http://api:7080/stats?period=month");
  });

  it("lists authors with defaults", async () => {
    await fetchAuthors();
    expect(lastCall().url).toBe("http://api:7080/authors?page=1&limit=20");

    await fetchAuthors("miura", 2, 10, "name");
    expect(lastCall().url).toBe("http://api:7080/authors?q=miura&sort=name&page=2&limit=10");
  });
});

describe("settings", () => {
  it("updates a setting", async () => {
    await updateSetting("downloads_enabled", true);
    expect(lastCall()).toMatchObject({
      url: "http://api:7080/settings/downloads_enabled",
      method: "POST",
      body: { value: true },
    });
  });

  it("reads the downloads flag", async () => {
    fetchMock.mockResolvedValue(makeResponse({ enabled: true }));
    await expect(fetchDownloadsEnabled()).resolves.toBe(true);

    fetchMock.mockResolvedValue(makeResponse({ enabled: false }));
    await expect(fetchDownloadsEnabled()).resolves.toBe(false);
  });

  it("returns false when the downloads flag cannot be read", async () => {
    fetchMock.mockRejectedValue(new Error("offline"));
    await expect(fetchDownloadsEnabled()).resolves.toBe(false);
  });
});

describe("komga", () => {
  it("posts the sync request as-is", async () => {
    const req = { url: "http://komga", username: "u", password: "p", user_id: "user-1" };
    await syncKomga(req);
    expect(lastCall()).toMatchObject({
      url: "http://api:7080/komga/sync",
      method: "POST",
      body: req,
    });
  });
});

describe("metadata gaps", () => {
  it("fetches the gap summary without a library filter", async () => {
    await fetchGapSummary();
    expect(lastCall().url).toBe("http://api:7080/metadata/gaps/summary");
    expect(lastCall().init.next).toEqual({ revalidate: 15, tags: ["metadata-gaps"] });
  });

  it("scopes the gap summary to a library", async () => {
    await fetchGapSummary("lib-1");
    expect(lastCall().url).toBe("http://api:7080/metadata/gaps/summary?library_id=lib-1");
  });
});
