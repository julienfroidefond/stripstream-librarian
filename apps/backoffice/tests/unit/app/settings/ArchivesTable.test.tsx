import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { ArchivesTable } from "@/app/(app)/settings/archives/ArchivesTable";
import type { ArchivedSeriesDetailDto, ArchivedSeriesItemDto } from "@/lib/api";

const fetchMock = vi.fn();

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

function makeSeries(overrides: Partial<ArchivedSeriesItemDto> = {}): ArchivedSeriesItemDto {
  return {
    id: "s1",
    name: "Astérix",
    library_id: "lib1",
    description: null,
    authors: ["Goscinny"],
    publishers: ["Dargaud"],
    genres: ["Humour"],
    cover_url: null,
    start_year: 1961,
    total_volumes: 3,
    status: "ongoing",
    book_count: 2,
    archived_at: "2024-03-05T10:00:00.000Z",
    reading_progress: [{ user_name: "bob", books_read: 1, books_reading: 0 }],
    ...overrides,
  };
}

function makeDetail(): ArchivedSeriesDetailDto {
  return {
    id: "s1",
    name: "Astérix",
    library_id: "lib1",
    description: "Une description",
    authors: ["Goscinny"],
    publishers: ["Dargaud"],
    genres: ["Humour"],
    cover_url: null,
    total_volumes: 3,
    status: "ongoing",
    start_year: 1961,
    archived_at: "2024-03-05T10:00:00.000Z",
    books: [
      {
        id: "b1",
        title: "Le Gaulois",
        kind: "comic",
        format: "cbz",
        volume: 1,
        volume_type: "regular",
        page_count: 48,
        thumbnail_path: null,
        author: "Goscinny",
        authors: ["Goscinny"],
        language: "fr",
        isbn: null,
        publish_date: "1961-10-29",
        abs_path: null,
        archived_at: "2024-03-05T10:00:00.000Z",
        reading_progress: [
          { user_name: "bob", status: "reading", current_page: 12, last_read_at: null },
        ],
      },
    ],
  };
}

describe("ArchivesTable", () => {
  it("renders translated headers and translates the series status", () => {
    render(<ArchivesTable series={[makeSeries()]} />);

    expect(screen.getByText("settings.archives.colSeries")).toBeInTheDocument();
    expect(screen.getByText("settings.archives.colArchivedAt")).toBeInTheDocument();
    // The raw enum must not leak through (it used to be rendered with `capitalize`).
    expect(screen.getByText("seriesStatus.ongoing")).toBeInTheDocument();
    expect(screen.queryByText("ongoing")).not.toBeInTheDocument();
  });

  it("fetches the detail on expand and renders volume + reading labels", async () => {
    fetchMock.mockResolvedValue({ ok: true, json: async () => makeDetail() });
    render(<ArchivesTable series={[makeSeries()]} />);

    fireEvent.click(screen.getByText("Astérix"));

    await waitFor(() => expect(screen.getByText("Le Gaulois")).toBeInTheDocument());
    expect(fetchMock).toHaveBeenCalledWith("/api/series/archived/s1");
    expect(screen.getByText('settings.archives.volume:{"volume":1}')).toBeInTheDocument();
    expect(screen.getByText("status.reading · p.12")).toBeInTheDocument();
  });

  it("shows the empty detail state when a series has no archived volume", async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      json: async () => ({ ...makeDetail(), books: [] }),
    });
    render(<ArchivesTable series={[makeSeries()]} />);

    fireEvent.click(screen.getByText("Astérix"));

    await waitFor(() => expect(screen.getByText("settings.archives.noBooks")).toBeInTheDocument());
  });

  it("reports a failed detail fetch", async () => {
    fetchMock.mockResolvedValue({ ok: false });
    render(<ArchivesTable series={[makeSeries()]} />);

    fireEvent.click(screen.getByText("Astérix"));

    await waitFor(() =>
      expect(screen.getByText("settings.archives.detailError")).toBeInTheDocument()
    );
  });
});
