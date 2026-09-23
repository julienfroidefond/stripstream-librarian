import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { MetadataBooksTable, MetadataSeriesTable } from "@/app/components/MetadataTable";
import type { BookDto, SeriesDto } from "@/lib/api";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";

const t = ((key: string) => key) as unknown as TranslateFunction;

function makeSeries(overrides: Partial<SeriesDto> = {}): SeriesDto {
  return {
    series_id: "series-1",
    name: "Series 1",
    book_count: 3,
    books_read_count: 1,
    first_book_id: null,
    first_book_updated_at: null,
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

function makeBook(overrides: Partial<BookDto> = {}): BookDto {
  return {
    id: "book-1",
    library_id: "lib-1",
    kind: "comic",
    format: null,
    title: "Book 1",
    author: null,
    authors: [],
    series: null,
    volume: null,
    volume_type: "regular",
    language: null,
    page_count: null,
    file_path: null,
    file_format: null,
    file_parse_status: null,
    updated_at: "2024-01-01T00:00:00.000Z",
    reading_status: "unread",
    reading_current_page: null,
    reading_last_read_at: null,
    series_id: null,
    summary: null,
    isbn: null,
    publish_date: null,
    ...overrides,
  };
}

describe("MetadataSeriesTable", () => {
  it("renders a header row for every metadata column", () => {
    render(<MetadataSeriesTable series={[makeSeries()]} t={t} knownStatuses={{}} />);

    for (const column of ["cover", "name", "provider", "status", "year", "genres", "volumes", "missing", "rating"]) {
      expect(screen.getByRole("columnheader", { name: `metadata.table.${column}` })).toBeInTheDocument();
    }
  });

  it("renders a row per series with a link to the series page", () => {
    render(
      <MetadataSeriesTable
        series={[
          makeSeries({ series_id: "s1", name: "Frieren" }),
          makeSeries({ series_id: "s2", name: "Berserk" }),
        ]}
        t={t}
        knownStatuses={{}}
      />
    );

    expect(screen.getByRole("link", { name: "Frieren" })).toHaveAttribute("href", "/series/s1");
    expect(screen.getByRole("link", { name: "Berserk" })).toHaveAttribute("href", "/series/s2");
  });

  it("displays provider, status, year, genres and counts", () => {
    render(
      <MetadataSeriesTable
        series={[
          makeSeries({
            name: "Frieren",
            metadata_provider: "comicvine",
            series_status: "ongoing",
            start_year: 2020,
            genres: ["Fantasy", "Aventure"],
            book_count: 12,
            missing_count: 3,
          }),
        ]}
        t={t}
        knownStatuses={{ ongoing: "Ongoing" }}
      />
    );

    expect(screen.getByText("comicvine")).toBeInTheDocument();
    expect(screen.getByText("Ongoing")).toBeInTheDocument();
    expect(screen.getByText("2020")).toBeInTheDocument();
    expect(screen.getByText("Fantasy")).toBeInTheDocument();
    expect(screen.getByText("Aventure")).toBeInTheDocument();
    expect(screen.getByText("12")).toBeInTheDocument();
    expect(screen.getByText("3")).toBeInTheDocument();
  });

  it("shows the user rating before the community score", () => {
    render(
      <MetadataSeriesTable
        series={[makeSeries({ user_rating: 4, community_score: 3.5 })]}
        t={t}
        knownStatuses={{}}
      />
    );

    expect(screen.getByText("★ 4")).toBeInTheDocument();
    expect(screen.queryByText("★ 3.5")).not.toBeInTheDocument();
  });

  it("falls back to empty markers when metadata is missing", () => {
    render(<MetadataSeriesTable series={[makeSeries()]} t={t} knownStatuses={{}} />);

    expect(screen.getAllByText("—").length).toBeGreaterThan(0);
  });
});

describe("MetadataBooksTable", () => {
  it("renders a header row for every metadata column", () => {
    render(<MetadataBooksTable books={[makeBook()]} t={t} />);

    for (const column of ["title", "series", "volume", "authors", "language", "pages", "format", "summary", "isbn"]) {
      expect(screen.getByRole("columnheader", { name: `metadata.table.${column}` })).toBeInTheDocument();
    }
  });

  it("renders a row per book with a link to the book page", () => {
    render(
      <MetadataBooksTable
        books={[
          makeBook({ id: "b1", title: "Alpha" }),
          makeBook({ id: "b2", title: "Beta" }),
        ]}
        t={t}
      />
    );

    expect(screen.getByRole("link", { name: "Alpha" })).toHaveAttribute("href", "/books/b1");
    expect(screen.getByRole("link", { name: "Beta" })).toHaveAttribute("href", "/books/b2");
  });

  it("shows a no-summary badge when the summary is missing", () => {
    render(<MetadataBooksTable books={[makeBook({ summary: null })]} t={t} />);

    expect(screen.getByText("metadata.gap.noSummary")).toBeInTheDocument();
  });

  it("hides the no-summary badge when the summary is present", () => {
    render(<MetadataBooksTable books={[makeBook({ summary: "A synopsis" })]} t={t} />);

    expect(screen.queryByText("metadata.gap.noSummary")).not.toBeInTheDocument();
  });

  it("displays series, volume, authors, language, pages, format and isbn", () => {
    render(
      <MetadataBooksTable
        books={[
          makeBook({
            title: "Alpha",
            series: "Frieren",
            volume: 2,
            authors: ["Abe", "Béa"],
            language: "fr",
            page_count: 180,
            format: "cbz",
            isbn: "9781234567890",
          }),
        ]}
        t={t}
      />
    );

    expect(screen.getByText("Frieren")).toBeInTheDocument();
    expect(screen.getByText("2")).toBeInTheDocument();
    expect(screen.getByText("Abe, Béa")).toBeInTheDocument();
    expect(screen.getByText("fr")).toBeInTheDocument();
    expect(screen.getByText("180")).toBeInTheDocument();
    expect(screen.getByText("cbz")).toBeInTheDocument();
    expect(screen.getByText("9781234567890")).toBeInTheDocument();
  });

  it("falls back to the unclassified label when there is no series", () => {
    render(<MetadataBooksTable books={[makeBook({ series: null })]} t={t} />);

    expect(screen.getByText("books.unclassified")).toBeInTheDocument();
  });
});
