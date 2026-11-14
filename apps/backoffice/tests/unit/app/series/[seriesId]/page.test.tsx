import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { SeriesDetailModel } from "@/lib/series/detail";

const loader = vi.hoisted(() => ({ loadSeriesDetail: vi.fn() }));

const cookiesState = vi.hoisted(() => ({ value: undefined as string | undefined }));

const notFoundMock = vi.hoisted(() =>
  vi.fn(() => {
    throw new Error("NEXT_NOT_FOUND");
  }),
);

const toolbar = vi.hoisted(() => ({ props: null as Record<string, unknown> | null }));
const toggle = vi.hoisted(() => ({ props: null as Record<string, unknown> | null }));
const pagination = vi.hoisted(() => ({ props: null as Record<string, unknown> | null }));

vi.mock("@/lib/series/detail", () => loader);

vi.mock("@/lib/api", () => ({
  getBookCoverUrl: (bookId: string, version?: string | null) =>
    version ? `/covers/${bookId}?v=${version}` : `/covers/${bookId}`,
}));

vi.mock("next/headers", () => ({
  cookies: async () => ({
    get: () => (cookiesState.value ? { value: cookiesState.value } : undefined),
  }),
}));

vi.mock("next/navigation", () => ({ notFound: notFoundMock }));

vi.mock("@/lib/i18n/server", () => ({
  getServerTranslations: async () => ({ t: (key: string) => key, locale: "en" }),
}));

vi.mock("@/app/components/SeriesActionsToolbar", () => ({
  SeriesActionsToolbar: (props: Record<string, unknown>) => {
    toolbar.props = props;
    return null;
  },
}));

vi.mock("@/app/components/BookCard", () => ({
  BooksGrid: () => null,
  BooksList: () => null,
  EmptyState: () => null,
}));

vi.mock("@/app/components/SeriesRelatedCarousel", () => ({
  SeriesRelatedCarousel: () => null,
}));

vi.mock("@/app/components/ShowMissingToggle", () => ({
  BooksGridWithMissingToggle: (props: Record<string, unknown>) => {
    toggle.props = props;
    return null;
  },
}));

vi.mock("@/app/components/ProviderIcon", () => ({
  ProviderIcon: () => null,
  providerLabel: (p: string) => p,
}));
vi.mock("@/app/components/SeriesRatingControl", () => ({
  SeriesRatingControl: () => null,
}));
vi.mock("@/app/components/SeriesUserActivity", () => ({
  SeriesUserRatings: () => null,
  SeriesUserReading: () => null,
}));
vi.mock("@/app/components/SeriesGrid", () => ({
  progressLabel: () => "",
}));
vi.mock("@/app/components/ui", () => ({
  OffsetPagination: (props: Record<string, unknown>) => {
    pagination.props = props;
    return null;
  },
}));
vi.mock("@/app/components/SafeHtml", () => ({ SafeHtml: () => null }));
vi.mock("@/app/components/ui/Icon", () => ({ Icon: () => null }));

import SeriesDetailPage from "@/app/(app)/series/[seriesId]/page";

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

function book(
  volume: number,
  volumeType: "regular" | "hs" | "oneshot" | "integral" = "regular",
  readingStatus: "unread" | "reading" | "read" = "unread",
) {
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
    volume_type: volumeType,
    language: "fr",
    page_count: 48,
    thumbnail_url: null,
    file_size: 1024,
    updated_at: "2026-01-01T00:00:00Z",
    reading_status: readingStatus,
    reading_current_page: null,
    reading_last_read_at: null,
    series_id: "ser-1",
    summary: null,
    isbn: null,
    publish_date: null,
    coverUrl: `/covers/book-${volume}`,
  };
}

function makeModel(overrides: Partial<SeriesDetailModel> = {}): SeriesDetailModel {
  return {
    seriesId: "ser-1",
    seriesName: "Series",
    series,
    library,
    page: 3,
    limit: 12,
    viewMode: "grid",
    books: [book(1, "regular", "read"), book(2, "regular"), book(3, "hs")],
    booksTotal: 5,
    totalPages: 1,
    booksReadCount: 1,
    hasActiveUser: true,
    activeUserId: "u1",
    metadata: {
      series_name: "Series",
      authors: ["Author"],
      genres: ["Action"],
      description: null,
      publishers: ["Publisher"],
      start_year: 2020,
      total_volumes: 10,
      status: "ongoing",
      book_author: null,
      book_language: null,
      locked_fields: {},
    },
    metadataLinks: [],
    existingLink: null,
    missingData: null,
    readingStatusLink: null,
    hiddenProviders: [],
    relatedSeries: [],
    readingLists: [
      {
        id: "list-1",
        name: "Favorites",
        description: null,
        series_count: 1,
        book_count: 3,
        books_read_count: 1,
        is_incomplete: false,
        preview_covers: [],
        created_at: "2026-01-01T00:00:00Z",
        updated_at: "2026-01-01T00:00:00Z",
      },
    ],
    ratings: null,
    ratedEntries: [],
    readingEntries: [],
    actions: {
      libraryId: "lib-1",
      seriesId: "ser-1",
      seriesName: "Series",
      bookCount: 5,
      booksReadCount: 1,
      hasActiveUser: true,
      isShelved: false,
      metadata: null,
      editBookAuthor: "Author",
      editBookLanguage: "fr",
      metadataLinks: [],
      existingLink: null,
      missingData: null,
      hiddenProviders: [],
      readingStatusProvider: "anilist",
      readingStatusLink: null,
      readiness: {
        downloads: true,
        prowlarr: true,
        qbittorrent: false,
        alldebrid: false,
        telegram: false,
        ddl: false,
      },
      ownedVolumes: [1, 2],
      renameFormats: { regular: null, hs: null, integral: null, oneshot: null },
    },
    degraded: [],
    ...overrides,
  };
}

beforeEach(() => {
  loader.loadSeriesDetail.mockReset();
  cookiesState.value = undefined;
  toolbar.props = null;
  toggle.props = null;
  pagination.props = null;
});

describe("SeriesDetailPage", () => {
  it("calls the single loader with the requested paging and renders its model", async () => {
    loader.loadSeriesDetail.mockResolvedValue({ status: "ok", model: makeModel() });

    render(
      await SeriesDetailPage({
        params: Promise.resolve({ seriesId: "ser-1" }),
        searchParams: Promise.resolve({ page: "3", limit: "12" }),
      }),
    );

    expect(loader.loadSeriesDetail).toHaveBeenCalledWith("ser-1", {
      page: 3,
      limit: 12,
      viewMode: "grid",
    });
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("Series");
    expect(screen.getByRole("link", { name: "Library" })).toBeInTheDocument();
    expect(screen.getByText("Author")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Favorites" })).toHaveAttribute(
      "href",
      "/reading-lists/list-1",
    );
  });

  it("falls back to the default page and limit", async () => {
    loader.loadSeriesDetail.mockResolvedValue({ status: "ok", model: makeModel() });

    render(
      await SeriesDetailPage({
        params: Promise.resolve({ seriesId: "ser-1" }),
        searchParams: Promise.resolve({}),
      }),
    );

    expect(loader.loadSeriesDetail).toHaveBeenCalledWith("ser-1", {
      page: 1,
      limit: 24,
      viewMode: "grid",
    });
  });

  it("forwards the view-mode cookie to the loader", async () => {
    cookiesState.value = "list";
    loader.loadSeriesDetail.mockResolvedValue({
      status: "ok",
      model: makeModel({ viewMode: "list" }),
    });

    render(
      await SeriesDetailPage({
        params: Promise.resolve({ seriesId: "ser-1" }),
        searchParams: Promise.resolve({}),
      }),
    );

    expect(loader.loadSeriesDetail).toHaveBeenCalledWith("ser-1", {
      page: 1,
      limit: 24,
      viewMode: "list",
    });
    expect(toggle.props?.viewMode).toBe("list");
  });

  it("hands the resolved actions model to the toolbar as its single prop", async () => {
    const model = makeModel();
    loader.loadSeriesDetail.mockResolvedValue({ status: "ok", model });

    render(
      await SeriesDetailPage({
        params: Promise.resolve({ seriesId: "ser-1" }),
        searchParams: Promise.resolve({}),
      }),
    );

    expect(Object.keys(toolbar.props ?? {})).toEqual(["model"]);
    expect(toolbar.props?.model).toBe(model.actions);
  });

  it("renders the regular volumes and the pagination from the model", async () => {
    loader.loadSeriesDetail.mockResolvedValue({ status: "ok", model: makeModel() });

    render(
      await SeriesDetailPage({
        params: Promise.resolve({ seriesId: "ser-1" }),
        searchParams: Promise.resolve({ page: "3", limit: "12" }),
      }),
    );

    const regularBooks = toggle.props?.books as Array<{ volume: number }>;
    expect(regularBooks.map((item) => item.volume)).toEqual([1, 2]);
    expect(screen.getByText("volumeType.hs")).toBeInTheDocument();

    expect(pagination.props).toEqual({
      currentPage: 3,
      totalPages: 1,
      pageSize: 12,
      totalItems: 5,
    });
  });

  it("renders the empty state when the series has no books", async () => {
    loader.loadSeriesDetail.mockResolvedValue({
      status: "ok",
      model: makeModel({ books: [], booksTotal: 0, totalPages: 0 }),
    });

    render(
      await SeriesDetailPage({
        params: Promise.resolve({ seriesId: "ser-1" }),
        searchParams: Promise.resolve({}),
      }),
    );

    expect(screen.queryByText("volumeType.hs")).not.toBeInTheDocument();
    expect(toggle.props).toBeNull();
  });

  it("maps a not-found result to notFound()", async () => {
    loader.loadSeriesDetail.mockResolvedValue({ status: "not-found" });

    await expect(
      SeriesDetailPage({
        params: Promise.resolve({ seriesId: "ser-1" }),
        searchParams: Promise.resolve({}),
      }),
    ).rejects.toThrow("NEXT_NOT_FOUND");

    expect(notFoundMock).toHaveBeenCalledTimes(1);
    expect(toolbar.props).toBeNull();
  });
});
