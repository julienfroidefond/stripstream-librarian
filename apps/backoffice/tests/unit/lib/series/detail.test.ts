// @vitest-environment node

import { beforeEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  apiFetch: vi.fn(),
  fetchBooks: vi.fn(),
  fetchLibraries: vi.fn(),
  fetchRelatedSeries: vi.fn(),
  fetchSeriesById: vi.fn(),
  fetchSeriesMetadata: vi.fn(),
  fetchSeriesOwnedVolumes: vi.fn(),
  fetchSeriesRatings: vi.fn(),
  fetchSeriesReadingLists: vi.fn(),
  fetchSeriesUserActivity: vi.fn(),
  getBookCoverUrl: vi.fn((bookId: string, version?: string | null) =>
    version ? `/covers/${bookId}?v=${encodeURIComponent(version)}` : `/covers/${bookId}`,
  ),
  getMetadataLink: vi.fn(),
  getMissingBooks: vi.fn(),
  getReadingStatusLink: vi.fn(),
}));

const activeUser = vi.hoisted(() => ({ getActiveUser: vi.fn() }));
const readinessLoader = vi.hoisted(() => ({ loadAcquisitionReadiness: vi.fn() }));

vi.mock("@/lib/api", () => api);
vi.mock("@/lib/auth/activeUser", () => activeUser);
vi.mock("@/lib/acquisition/readiness.server", () => readinessLoader);

import { loadSeriesDetail, type SeriesDetailSection } from "@/lib/series/detail";

const readiness = {
  downloads: true,
  prowlarr: true,
  qbittorrent: false,
  alldebrid: false,
  telegram: true,
  ddl: false,
};

const series = {
  series_id: "ser-1",
  name: "Series",
  book_count: 5,
  books_read_count: 1,
  last_read_at: null,
  first_book_id: "book-1",
  first_book_updated_at: "2026-01-01T00:00:00Z",
  library_id: "lib-1",
  series_status: "ongoing",
  missing_count: 3,
  metadata_provider: "comicvine",
  anilist_id: null,
  anilist_url: null,
  cover_url: null,
  start_year: 2020,
  genres: ["Action"],
  user_rating: null,
  community_score: null,
  shelved_at: null,
};

const library = {
  id: "lib-1",
  name: "Library",
  root_path: "/libraries/main",
  enabled: true,
  book_count: 5,
  monitor_enabled: false,
  scan_mode: "manual",
  next_scan_at: null,
  watcher_enabled: false,
  metadata_provider: "comicvine",
  fallback_metadata_provider: null,
  metadata_refresh_mode: "manual",
  next_metadata_refresh_at: null,
  series_count: 1,
  thumbnail_book_ids: [],
  reading_status_provider: "anilist",
  reading_status_push_mode: "manual",
  next_reading_status_push_at: null,
  download_detection_mode: "manual",
  next_download_detection_at: null,
  tags: [],
};

const metadata = {
  series_name: "Series",
  authors: ["Author"],
  genres: ["Action"],
  description: "<p>Description</p>",
  publishers: ["Publisher"],
  start_year: 2020,
  total_volumes: 10,
  status: "ongoing",
  book_author: null,
  book_language: null,
  locked_fields: {},
};

const approvedLink = {
  id: "link-1",
  library_id: "lib-1",
  series_name: "Series",
  provider: "comicvine",
  external_id: "4050-1",
  external_url: "https://comicvine.example/series",
  status: "approved",
  is_primary: true,
  confidence: 0.95,
  metadata_json: {},
  total_volumes_external: 10,
  matched_at: "2026-01-01T00:00:00Z",
  approved_at: "2026-01-01T00:00:00Z",
  synced_at: null,
};

const missingBooks = {
  total_external: 10,
  total_local: 7,
  missing_count: 3,
  missing_books: [{ title: "Vol 8", volume_number: 8, external_book_id: "1", cover_url: null }],
};

const ratings = {
  user_rating: 4,
  anilist_pulled_rating: null,
  provider_ratings: [{ provider: "comicvine", rating: 4.2, rating_scale: 5, rating_count: 120 }],
};

const readingList = { id: "list-1", name: "Favorites", description: null, item_count: 3 };

const relatedSeries = [
  {
    series_id: "ser-2",
    name: "Related",
    book_count: 2,
    books_read_count: 0,
    first_book_id: "book-9",
    first_book_updated_at: null,
    library_id: "lib-1",
    series_status: null,
    metadata_provider: null,
    cover_url: null,
    score: 1,
    match_reasons: ["author"],
  },
];

const userActivity = [
  {
    user_id: "u1",
    username: "Alice",
    books_read: 1,
    books_reading: 0,
    books_total: 5,
    last_read_at: "2026-01-01T00:00:00Z",
    rating: 4,
  },
  {
    user_id: "u2",
    username: "Bob",
    books_read: 0,
    books_reading: 1,
    books_total: 5,
    last_read_at: null,
    rating: null,
  },
];

function book(volume: number, overrides: Record<string, unknown> = {}) {
  return {
    id: `book-${volume}`,
    library_id: "lib-1",
    kind: "comic",
    format: "cbz",
    title: `Vol ${volume}`,
    author: "Author",
    authors: ["Author"],
    series: "Series",
    volume,
    volume_type: "regular",
    language: "fr",
    page_count: 48,
    file_path: `/libraries/main/Series/${volume}.cbz`,
    file_format: "cbz",
    file_parse_status: "ok",
    file_size: 1024,
    updated_at: "2026-01-01T00:00:00Z",
    reading_status: volume === 1 ? "read" : "unread",
    reading_current_page: null,
    reading_last_read_at: null,
    series_id: "ser-1",
    summary: null,
    isbn: null,
    publish_date: null,
    ...overrides,
  };
}

function settingsValue(url: string): unknown {
  switch (url) {
    case "/settings/metadata_providers":
      return { comicvine: { api_key: "key" } };
    case "/settings/rename_format":
      return "{series} - {volume}";
    case "/settings/rename_format_hs":
      return "HS {series}";
    case "/settings/rename_format_int":
      return "INT {series}";
    case "/settings/rename_format_oneshot":
      return "OS {series}";
    default:
      throw new Error(`unexpected settings fetch: ${url}`);
  }
}

const defaultOptions = { page: 2, limit: 48, viewMode: "list" as const };

beforeEach(() => {
  api.fetchSeriesById.mockReset().mockResolvedValue(series);
  api.fetchLibraries.mockReset().mockResolvedValue([library]);
  api.fetchBooks.mockReset().mockResolvedValue({
    items: [book(1), book(2)],
    total: 5,
    page: 2,
    limit: 48,
  });
  api.fetchSeriesMetadata.mockReset().mockResolvedValue(metadata);
  api.getMetadataLink.mockReset().mockResolvedValue([approvedLink]);
  api.getReadingStatusLink.mockReset().mockResolvedValue(null);
  api.apiFetch.mockReset().mockImplementation(async (url: string) => settingsValue(url));
  api.fetchRelatedSeries.mockReset().mockResolvedValue(relatedSeries);
  api.fetchSeriesReadingLists.mockReset().mockResolvedValue([readingList]);
  api.fetchSeriesRatings.mockReset().mockResolvedValue(ratings);
  api.fetchSeriesUserActivity.mockReset().mockResolvedValue(userActivity);
  api.fetchSeriesOwnedVolumes.mockReset().mockResolvedValue([1, 2]);
  api.getMissingBooks.mockReset().mockResolvedValue(missingBooks);
  activeUser.getActiveUser.mockReset().mockResolvedValue({ userId: "u1", hasActiveUser: true });
  readinessLoader.loadAcquisitionReadiness.mockReset().mockResolvedValue(readiness);
});

describe("loadSeriesDetail", () => {
  it("assembles the whole page model and forwards pagination and view mode", async () => {
    const result = await loadSeriesDetail("ser-1", defaultOptions);

    expect(result.status).toBe("ok");
    if (result.status !== "ok") return;
    const { model } = result;

    expect(api.fetchBooks).toHaveBeenCalledWith("lib-1", "ser-1", 2, 48);
    expect(api.fetchRelatedSeries).toHaveBeenCalledWith("ser-1", 12);
    expect(api.getMissingBooks).toHaveBeenCalledWith("link-1");

    expect(model.seriesId).toBe("ser-1");
    expect(model.seriesName).toBe("Series");
    expect(model.library).toEqual(library);
    expect(model.page).toBe(2);
    expect(model.limit).toBe(48);
    expect(model.viewMode).toBe("list");
    expect(model.booksTotal).toBe(5);
    expect(model.totalPages).toBe(1);
    expect(model.booksReadCount).toBe(1);
    expect(model.books[0].coverUrl).toBe("/covers/book-1?v=2026-01-01T00%3A00%3A00Z");
    expect(model.degraded).toEqual([]);
  });

  it("takes the read count from the whole series, not the current page", async () => {
    // Page 2 holds one read book, but 4 of the 5 books of the series are read.
    api.fetchSeriesById.mockResolvedValue({ ...series, books_read_count: 4 });

    const result = await loadSeriesDetail("ser-1", defaultOptions);

    expect(result.status).toBe("ok");
    if (result.status !== "ok") return;
    expect(result.model.booksReadCount).toBe(4);
  });

  it("resolves the metadata, links, ratings and collection sections once", async () => {
    const result = await loadSeriesDetail("ser-1", defaultOptions);

    expect(result.status).toBe("ok");
    if (result.status !== "ok") return;
    const { model } = result;

    expect(model.metadata).toEqual(metadata);
    expect(model.metadataLinks).toEqual([approvedLink]);
    expect(model.existingLink).toEqual(approvedLink);
    expect(model.missingData).toEqual(missingBooks);
    expect(model.relatedSeries).toEqual(relatedSeries);
    expect(model.readingLists).toEqual([readingList]);
    expect(model.ratings).toEqual(ratings);
    expect(model.actions.ownedVolumes).toEqual([1, 2]);
    expect(model.hiddenProviders).toEqual([]);
  });

  it("splits reader activity into rating and reading entries in admin mode", async () => {
    activeUser.getActiveUser.mockResolvedValue({ userId: null, hasActiveUser: false });

    const result = await loadSeriesDetail("ser-1", defaultOptions);

    expect(result.status).toBe("ok");
    if (result.status !== "ok") return;

    expect(result.model.ratedEntries.map((entry) => entry.username)).toEqual(["Alice"]);
    expect(result.model.readingEntries.map((entry) => entry.username)).toEqual(["Alice", "Bob"]);
  });

  it("derives a complete actions model for the toolbar", async () => {
    const result = await loadSeriesDetail("ser-1", defaultOptions);

    expect(result.status).toBe("ok");
    if (result.status !== "ok") return;
    const { actions } = result.model;

    expect(actions).toMatchObject({
      libraryId: "lib-1",
      seriesId: "ser-1",
      seriesName: "Series",
      bookCount: 5,
      booksReadCount: 1,
      hasActiveUser: true,
      isShelved: false,
      metadata,
      editBookAuthor: "Author",
      editBookLanguage: "fr",
      existingLink: approvedLink,
      missingData: missingBooks,
      hiddenProviders: [],
      readingStatusProvider: "anilist",
      readiness,
      ownedVolumes: [1, 2],
    });
    expect(actions.renameFormats).toEqual({
      regular: "{series} - {volume}",
      hs: "HS {series}",
      integral: "INT {series}",
      oneshot: "OS {series}",
    });
  });

  it("returns not-found when the series cannot be resolved", async () => {
    api.fetchSeriesById.mockRejectedValue(new Error("404"));

    await expect(loadSeriesDetail("ser-1", defaultOptions)).resolves.toEqual({
      status: "not-found",
    });
    expect(api.fetchBooks).not.toHaveBeenCalled();
  });

  it("returns not-found when the series library is unknown", async () => {
    api.fetchLibraries.mockResolvedValue([]);

    await expect(loadSeriesDetail("ser-1", defaultOptions)).resolves.toEqual({
      status: "not-found",
    });
  });

  it("degrades the books page instead of failing the series", async () => {
    api.fetchBooks.mockRejectedValue(new Error("books down"));

    const result = await loadSeriesDetail("ser-1", defaultOptions);

    expect(result.status).toBe("ok");
    if (result.status !== "ok") return;
    expect(result.model.books).toEqual([]);
    expect(result.model.booksTotal).toBe(0);
    expect(result.model.degraded).toContain("books");
  });

  describe("degradation", () => {
    const cases: Array<[string, SeriesDetailSection, () => void]> = [
      [
        "metadata",
        "metadata",
        () => api.fetchSeriesMetadata.mockRejectedValue(new Error("down")),
      ],
      ["metadata links", "metadataLinks", () => api.getMetadataLink.mockRejectedValue(new Error("down"))],
      [
        "reading status link",
        "readingStatusLink",
        () => api.getReadingStatusLink.mockRejectedValue(new Error("down")),
      ],
      [
        "metadata providers",
        "metadataProviders",
        () =>
          api.apiFetch.mockImplementation(async (url: string) => {
            if (url === "/settings/metadata_providers") throw new Error("down");
            return settingsValue(url);
          }),
      ],
      [
        "rename formats",
        "renameFormats",
        () =>
          api.apiFetch.mockImplementation(async (url: string) => {
            if (url === "/settings/rename_format_hs") throw new Error("down");
            return settingsValue(url);
          }),
      ],
      [
        "related series",
        "relatedSeries",
        () => api.fetchRelatedSeries.mockRejectedValue(new Error("down")),
      ],
      [
        "reading lists",
        "readingLists",
        () => api.fetchSeriesReadingLists.mockRejectedValue(new Error("down")),
      ],
      ["ratings", "ratings", () => api.fetchSeriesRatings.mockRejectedValue(new Error("down"))],
      [
        "owned volumes",
        "ownedVolumes",
        () => api.fetchSeriesOwnedVolumes.mockRejectedValue(new Error("down")),
      ],
      [
        "missing books",
        "missingBooks",
        () => api.getMissingBooks.mockRejectedValue(new Error("down")),
      ],
    ];

    it.each(cases)("records a failing %s section and keeps the page", async (_label, section, fail) => {
      fail();

      const result = await loadSeriesDetail("ser-1", defaultOptions);

      expect(result.status).toBe("ok");
      if (result.status !== "ok") return;
      expect(result.model.degraded).toContain(section);
    });

    it("records a failing reader activity section in admin mode", async () => {
      activeUser.getActiveUser.mockResolvedValue({ userId: null, hasActiveUser: false });
      api.fetchSeriesUserActivity.mockRejectedValue(new Error("down"));

      const result = await loadSeriesDetail("ser-1", defaultOptions);

      expect(result.status).toBe("ok");
      if (result.status !== "ok") return;
      expect(result.model.degraded).toEqual(["userActivity"]);
      expect(result.model.ratedEntries).toEqual([]);
      expect(result.model.readingEntries).toEqual([]);
    });

    it("degrades a failing metadata section to empty defaults", async () => {
      api.fetchSeriesMetadata.mockRejectedValue(new Error("down"));
      api.getMetadataLink.mockRejectedValue(new Error("down"));

      const result = await loadSeriesDetail("ser-1", defaultOptions);

      expect(result.status).toBe("ok");
      if (result.status !== "ok") return;
      expect(result.model.degraded).toEqual(["metadata", "metadataLinks"]);
      expect(result.model.metadata).toBeNull();
      expect(result.model.metadataLinks).toEqual([]);
      expect(result.model.existingLink).toBeNull();
      expect(result.model.missingData).toBeNull();
      expect(result.model.actions.metadata).toBeNull();
    });
  });

  it("does not query reader activity when a reader is selected", async () => {
    const result = await loadSeriesDetail("ser-1", defaultOptions);

    expect(api.fetchSeriesUserActivity).not.toHaveBeenCalled();

    expect(result.status).toBe("ok");
    if (result.status !== "ok") return;
    expect(result.model.ratedEntries).toEqual([]);
    expect(result.model.readingEntries).toEqual([]);
  });

  it("queries reader activity in admin mode and skips missing books without an approved link", async () => {
    activeUser.getActiveUser.mockResolvedValue({ userId: null, hasActiveUser: false });
    api.getMetadataLink.mockResolvedValue([{ ...approvedLink, status: "pending" }]);

    const result = await loadSeriesDetail("ser-1", defaultOptions);

    expect(api.fetchSeriesUserActivity).toHaveBeenCalledWith("ser-1");
    expect(api.getMissingBooks).not.toHaveBeenCalled();

    expect(result.status).toBe("ok");
    if (result.status !== "ok") return;
    expect(result.model.degraded).toEqual([]);
    expect(result.model.missingData).toBeNull();
    expect(result.model.activeUserId).toBe("admin");
  });
});
