import { render } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  fetchLibraries: vi.fn(),
  fetchBooks: vi.fn(),
  fetchSeriesMetadata: vi.fn(),
  fetchSeriesById: vi.fn(),
  fetchSeriesOwnedVolumes: vi.fn(),
  fetchRelatedSeries: vi.fn(),
  getBookCoverUrl: vi.fn((bookId: string) => `/covers/${bookId}.webp`),
  getMetadataLink: vi.fn(),
  getMissingBooks: vi.fn(),
  getReadingStatusLink: vi.fn(),
  apiFetch: vi.fn(),
  fetchSeriesReadingLists: vi.fn(),
  fetchSeriesRatings: vi.fn(),
  fetchSeriesUserActivity: vi.fn(),
}));

const readiness = vi.hoisted(() => ({
  downloads: true,
  prowlarr: true,
  qbittorrent: true,
  alldebrid: false,
  telegram: false,
  ddl: true,
}));

const toolbar = vi.hoisted(() => ({ props: null as Record<string, unknown> | null }));

vi.mock("@/lib/api", () => api);

vi.mock("@/lib/acquisitionReadiness.server", () => ({
  loadAcquisitionReadiness: async () => readiness,
}));

vi.mock("next/headers", () => ({
  cookies: async () => ({ get: () => undefined }),
}));

vi.mock("@/lib/i18n/server", () => ({
  getServerTranslations: async () => ({ t: (key: string) => key, locale: "en" }),
}));

vi.mock("@/lib/activeUser", () => ({
  getActiveUser: async () => ({ userId: "u1", hasActiveUser: true }),
}));

vi.mock("@/lib/userActivity", () => ({
  ratedActivity: () => [],
  readingActivity: () => [],
}));

vi.mock("@/lib/viewMode", () => ({
  parseViewMode: () => "grid",
  VIEW_MODE_COOKIE: "view-mode",
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
  BooksGridWithMissingToggle: () => null,
}));
vi.mock("@/app/components/ViewModeToggle", () => ({ ViewModeToggle: () => null }));
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
vi.mock("@/app/components/ui", () => ({ OffsetPagination: () => null }));
vi.mock("@/app/components/SafeHtml", () => ({ SafeHtml: () => null }));
vi.mock("@/app/components/ui/Icon", () => ({ Icon: () => null }));

import SeriesDetailPage from "@/app/(app)/series/[seriesId]/page";

function book(volume: number) {
  return {
    id: `book-${volume}`,
    title: `Vol ${volume}`,
    volume,
    volume_type: "regular",
    reading_status: "unread",
    updated_at: "2026-01-01T00:00:00Z",
  };
}

beforeEach(() => {
  api.fetchLibraries.mockReset().mockResolvedValue([{ id: "lib-1", name: "Library" }]);
  api.fetchBooks.mockReset().mockResolvedValue({
    items: [book(1)],
    total: 1,
    page: 1,
    limit: 24,
  });
  api.fetchSeriesMetadata.mockReset().mockResolvedValue({
    series_name: "Series",
    authors: [],
    genres: [],
    description: null,
    publishers: [],
    start_year: null,
    total_volumes: null,
    status: null,
    book_author: null,
    book_language: null,
    locked_fields: {},
  });
  api.fetchSeriesById.mockReset().mockResolvedValue({
    series_id: "ser-1",
    library_id: "lib-1",
    name: "Series",
    first_book_id: null,
    first_book_updated_at: null,
    cover_url: null,
    shelved_at: null,
  });
  api.fetchSeriesOwnedVolumes.mockReset().mockResolvedValue([1, 2, 3]);
  api.fetchRelatedSeries.mockReset().mockResolvedValue([]);
  api.getMetadataLink.mockReset().mockResolvedValue([]);
  api.getReadingStatusLink.mockReset().mockResolvedValue(null);
  api.apiFetch.mockReset().mockResolvedValue(null);
  api.fetchSeriesReadingLists.mockReset().mockResolvedValue([]);
  api.fetchSeriesRatings.mockReset().mockResolvedValue(null);
  api.fetchSeriesUserActivity.mockReset().mockResolvedValue([]);
  toolbar.props = null;
});

describe("SeriesDetailPage", () => {
  it("passes the complete owned-volume list to the toolbar, not just the current page", async () => {
    render(
      await SeriesDetailPage({
        params: Promise.resolve({ seriesId: "ser-1" }),
        searchParams: Promise.resolve({}),
      })
    );

    expect(api.fetchSeriesOwnedVolumes).toHaveBeenCalledWith("ser-1");
    expect(toolbar.props?.ownedVolumes).toEqual([1, 2, 3]);
    expect(toolbar.props?.readiness).toEqual(readiness);
  });

  it("falls back to undefined so a failed fetch highlights nothing", async () => {
    api.fetchSeriesOwnedVolumes.mockRejectedValue(new Error("boom"));

    render(
      await SeriesDetailPage({
        params: Promise.resolve({ seriesId: "ser-1" }),
        searchParams: Promise.resolve({}),
      })
    );

    expect(toolbar.props?.ownedVolumes).toBeUndefined();
  });

  it("passes undefined when the series owns an unnumbered integral", async () => {
    api.fetchSeriesOwnedVolumes.mockResolvedValue(null);

    render(
      await SeriesDetailPage({
        params: Promise.resolve({ seriesId: "ser-1" }),
        searchParams: Promise.resolve({}),
      })
    );

    expect(toolbar.props?.ownedVolumes).toBeUndefined();
  });

  it("passes the active reader's shelf state to the toolbar", async () => {
    api.fetchSeriesById.mockResolvedValue({
      series_id: "ser-1",
      library_id: "lib-1",
      name: "Series",
      first_book_id: null,
      first_book_updated_at: null,
      cover_url: null,
      shelved_at: "2026-01-01T00:00:00Z",
    });

    render(
      await SeriesDetailPage({
        params: Promise.resolve({ seriesId: "ser-1" }),
        searchParams: Promise.resolve({}),
      })
    );

    expect(toolbar.props?.initialIsShelved).toBe(true);
  });
});
