import { render } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  fetchLibraries: vi.fn(),
  getBookCoverUrl: vi.fn((bookId: string) => `/covers/${bookId}.webp`),
  apiFetch: vi.fn(),
  fetchSeriesOwnedVolumes: vi.fn(),
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

vi.mock("@/lib/activeUser", () => ({
  getActiveUser: async () => ({ userId: "u1", hasActiveUser: true }),
}));

vi.mock("@/lib/i18n/server", () => ({
  getServerTranslations: async () => ({ t: (key: string) => key, locale: "en" }),
}));

vi.mock("@/app/components/BookActionsToolbar", () => ({
  BookActionsToolbar: (props: Record<string, unknown>) => {
    toolbar.props = props;
    return null;
  },
}));

vi.mock("@/app/components/BookPreview", () => ({ BookPreview: () => null }));
vi.mock("@/app/components/SafeHtml", () => ({ SafeHtml: () => null }));

import BookDetailPage from "@/app/(app)/books/[id]/page";

function book(overrides: Record<string, unknown> = {}) {
  return {
    id: "book-1",
    library_id: "lib-1",
    kind: "comic",
    format: "cbz",
    title: "Vol 1",
    author: null,
    authors: [],
    series: "Series",
    volume: 1,
    volume_type: "regular",
    language: null,
    page_count: null,
    file_path: null,
    file_format: null,
    file_parse_status: null,
    file_size: null,
    updated_at: "2026-01-01T00:00:00Z",
    reading_status: "unread",
    reading_current_page: null,
    reading_last_read_at: null,
    series_id: "ser-1",
    summary: null,
    isbn: null,
    publish_date: null,
    ...overrides,
  };
}

function renderBook() {
  return BookDetailPage({
    params: Promise.resolve({ id: "book-1" }),
  });
}

beforeEach(() => {
  api.fetchLibraries.mockReset().mockResolvedValue([{ id: "lib-1", name: "Library" }]);
  api.apiFetch.mockReset().mockImplementation(async (url: string) =>
    url === "/books/book-1" ? book() : null
  );
  api.fetchSeriesOwnedVolumes.mockReset().mockResolvedValue([1, 2, 3]);
  toolbar.props = null;
});

describe("BookDetailPage", () => {
  it("forwards the fetched owned volumes to the toolbar", async () => {
    render(await renderBook());

    expect(api.fetchSeriesOwnedVolumes).toHaveBeenCalledWith("ser-1");
    expect(toolbar.props?.ownedVolumes).toEqual([1, 2, 3]);
    expect(toolbar.props?.readiness).toEqual(readiness);
  });

  it("forwards undefined (highlight nothing) when the book has no series", async () => {
    api.apiFetch.mockImplementation(async (url: string) =>
      url === "/books/book-1" ? book({ series_id: null, series: null }) : null
    );

    render(await renderBook());

    expect(api.fetchSeriesOwnedVolumes).not.toHaveBeenCalled();
    expect(toolbar.props?.ownedVolumes).toBeUndefined();
  });

  it("forwards undefined when the owned-volume fetch fails", async () => {
    api.fetchSeriesOwnedVolumes.mockRejectedValue(new Error("boom"));

    render(await renderBook());

    expect(toolbar.props?.ownedVolumes).toBeUndefined();
  });
});
