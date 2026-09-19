import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { SeriesGrid } from "@/app/components/SeriesGrid";
import type { SeriesDto } from "@/lib/api";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";

function makeSeries(overrides: Partial<SeriesDto> = {}): SeriesDto {
  return {
    series_id: "series-1",
    name: "Series 1",
    book_count: 3,
    books_read_count: 1,
    first_book_id: "book-1",
    first_book_updated_at: "2024-01-01T00:00:00.000Z",
    library_id: "lib-1",
    series_status: null,
    missing_count: null,
    metadata_provider: null,
    anilist_id: null,
    anilist_url: null,
    cover_url: null,
    start_year: null,
    genres: [],
    user_rating: null,
    community_score: null,
    ...overrides,
  };
}

const t = ((key: string) => key) as unknown as TranslateFunction;

function renderGrid(series: SeriesDto[], overrides: Partial<React.ComponentProps<typeof SeriesGrid>> = {}) {
  render(
    <SeriesGrid
      series={series}
      hasActiveUser={false}
      t={t}
      knownStatuses={{}}
      {...overrides}
    />
  );
}

describe("SeriesGrid", () => {
  it("renders a card with a link per series", () => {
    renderGrid([makeSeries({ series_id: "s1", name: "Frieren" }), makeSeries({ series_id: "s2", name: "Berserk" })]);

    expect(screen.getByRole("link", { name: "Frieren" })).toHaveAttribute("href", "/series/s1");
    expect(screen.getByRole("link", { name: "Berserk" })).toHaveAttribute("href", "/series/s2");
  });

  it("inserts year section headers when sorted by release date", () => {
    renderGrid(
      [makeSeries({ series_id: "s1", name: "A", start_year: 2020 }), makeSeries({ series_id: "s2", name: "B", start_year: 2021 })],
      { sort: "release_date" }
    );

    expect(screen.getByText("2020")).toBeInTheDocument();
    expect(screen.getByText("2021")).toBeInTheDocument();
  });

  it("does not show year headers for other sorts", () => {
    renderGrid([makeSeries({ start_year: 2020 })]);

    expect(screen.queryByText("2020")).not.toBeInTheDocument();
  });

  it("groups series by community score when requested", () => {
    renderGrid(
      [
        makeSeries({ series_id: "s1", name: "A", community_score: 4 }),
        makeSeries({ series_id: "s2", name: "B", community_score: 4 }),
        makeSeries({ series_id: "s3", name: "C", community_score: 2 }),
      ],
      { sort: "community_score", showCommunityScore: true }
    );

    expect(screen.getByText("★★★★☆ 4/5")).toBeInTheDocument();
    expect(screen.getByText("★★☆☆☆ 2/5")).toBeInTheDocument();
  });

  it("exposes a mark-read control only when a user is active", () => {
    const { unmount } = render(
      <SeriesGrid
        series={[makeSeries({ name: "Solo" })]}
        hasActiveUser
        t={t}
        knownStatuses={{}}
      />
    );

    expect(screen.getByTitle("markRead.read")).toBeInTheDocument();
    unmount();

    renderGrid([makeSeries({ name: "Solo" })]);
    expect(screen.queryByTitle("markRead.read")).not.toBeInTheDocument();
  });

  it("translates the unclassified series name", () => {
    renderGrid([makeSeries({ name: "unclassified" })]);

    expect(screen.getByText("books.unclassified")).toBeInTheDocument();
  });

  it("shows a badge for the series status", () => {
    renderGrid([makeSeries({ series_status: "ongoing" })], {
      knownStatuses: { ongoing: "Ongoing" },
    });

    expect(screen.getByText("Ongoing")).toBeInTheDocument();
  });

  it("shows the missing books count", () => {
    renderGrid([makeSeries({ missing_count: 3 })]);

    expect(screen.getByText("3")).toBeInTheDocument();
  });

  it("calls translate for the read count", () => {
    const spy = vi.fn((key: string) => key);
    render(
      <SeriesGrid
        series={[makeSeries({ book_count: 4, books_read_count: 2 })]}
        hasActiveUser={false}
        t={spy as unknown as TranslateFunction}
        knownStatuses={{}}
      />
    );

    expect(spy).toHaveBeenCalledWith(
      "series.readCount",
      expect.objectContaining({ read: "2", total: "4" })
    );
  });
});
