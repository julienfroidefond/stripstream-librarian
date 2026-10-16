import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { ReadingOverviewTab } from "@/app/(app)/settings/components/ReadingOverviewTab";

const fetchMock = vi.fn();

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

const overview = [
  {
    user_id: "u1",
    username: "Alice",
    books_read: 3,
    books_reading: 1,
    series_in_progress: 1,
    last_read_at: "2024-01-01",
    currently_reading: [
      { book_id: "b1", title: "Book1", series: "Series1", current_page: 5, page_count: 10 },
      { book_id: "b2", title: "Book2", series: null, current_page: 0, page_count: 0 },
    ],
    series_progress: [
      {
        series_id: "s1",
        series_name: "Serie1",
        first_book_id: "b1",
        books_total: 3,
        books_read: 2,
        books_reading: 1,
        books_unread: 0,
        last_read_at: "2024-01-02",
      },
      {
        series_id: null,
        series_name: "Unclassified",
        first_book_id: null,
        books_total: 0,
        books_read: 0,
        books_reading: 0,
        books_unread: 0,
        last_read_at: null,
      },
    ],
    recently_read: [
      { book_id: "b9", title: "R1", series: "S", last_read_at: "2024-01-03" },
      { book_id: "b10", title: "R2", series: null, last_read_at: null },
    ],
  },
  {
    user_id: "u2",
    username: "Bob",
    books_read: 0,
    books_reading: 0,
    series_in_progress: 0,
    last_read_at: null,
    currently_reading: [],
  },
];

const serieBooks = [
  { book_id: "b1", title: "V1", volume: 1, volume_type: "regular", status: "read", current_page: 0, page_count: 100, last_read_at: "2024-01-02" },
  { book_id: "b2", title: "V2", volume: null, volume_type: "hs", status: "reading", current_page: 10, page_count: 20, last_read_at: null },
  { book_id: "b3", title: "V3", volume: 2, volume_type: "oneshot", status: "unread", current_page: 0, page_count: 50, last_read_at: "x" },
  { book_id: "b4", title: "V4", volume: 3, volume_type: "integral", status: "read", current_page: 0, page_count: 0, last_read_at: null },
];

function mockOverview(payload: unknown, seriesBooks: unknown = serieBooks) {
  fetchMock.mockImplementation(async (url: string) => {
    if (url.includes("/series-books")) {
      return { ok: true, json: async () => seriesBooks };
    }
    return { ok: true, json: async () => payload };
  });
}

describe("ReadingOverviewTab", () => {
  it("renders the selected user's overview and expands series volumes", async () => {
    mockOverview(overview);
    render(<ReadingOverviewTab />);

    expect(await screen.findByText("Alice")).toBeInTheDocument();
    expect(screen.getByText("Bob")).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /Alice/ })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByText("Book1")).toBeInTheDocument();
    expect(screen.getByText("Book2")).toBeInTheDocument();
    expect(screen.getByText("Serie1")).toBeInTheDocument();
    expect(screen.getByText("Unclassified")).toBeInTheDocument();
    expect(screen.getByText("R1")).toBeInTheDocument();

    expect(fetchMock).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByText("Serie1"));
    expect(screen.getAllByTestId("volume-skeleton").length).toBeGreaterThan(0);
    expect(await screen.findByText("V1")).toBeInTheDocument();
    expect(fetchMock).toHaveBeenCalledWith("/api/reading-overview/u1/series-books?series_id=s1");
    expect(screen.getByText("T1")).toBeInTheDocument();
    expect(screen.getByText("OS")).toBeInTheDocument();
    expect(screen.getByText("Int 3")).toBeInTheDocument();
    expect(screen.getByText(/10\/20/)).toBeInTheDocument();

    fireEvent.click(screen.getByText("Serie1"));
    expect(screen.queryByText("V1")).not.toBeInTheDocument();

    // Re-expanding reuses the already-loaded books.
    fireEvent.click(screen.getByText("Serie1"));
    expect(screen.getByText("V1")).toBeInTheDocument();
    expect(fetchMock).toHaveBeenCalledTimes(2);

    fireEvent.click(screen.getByText("Unclassified"));
    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith("/api/reading-overview/u1/series-books"),
    );

    fireEvent.click(screen.getByRole("tab", { name: /Bob/ }));
    expect(await screen.findByText("settings.readingOverview.noSeries")).toBeInTheDocument();
    expect(screen.queryByText("Book1")).not.toBeInTheDocument();
  });

  it("switches the series statuses between list and grid views", async () => {
    mockOverview(overview);
    render(<ReadingOverviewTab />);

    expect(await screen.findByText("Serie1")).toBeInTheDocument();
    expect(screen.getByTestId("series-list")).toBeInTheDocument();
    expect(screen.queryByTestId("series-grid")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "settings.readingOverview.viewGrid" }));
    expect(screen.getByTestId("series-grid")).toBeInTheDocument();
    expect(screen.queryByTestId("series-list")).not.toBeInTheDocument();
    expect(screen.getByText("Serie1")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "settings.readingOverview.viewGrid" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );

    // Cards stay expandable to reveal the series volumes.
    fireEvent.click(screen.getByText("Serie1"));
    expect(await screen.findByText("V1")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "settings.readingOverview.viewList" }));
    expect(screen.getByTestId("series-list")).toBeInTheDocument();
    expect(screen.queryByTestId("series-grid")).not.toBeInTheDocument();
  });

  it("collapses long series lists behind a show-all toggle", async () => {
    const series = Array.from({ length: 10 }, (_, i) => ({
      series_id: `s${i}`,
      series_name: `Series ${i}`,
      books_total: 1,
      books_read: 0,
      books_reading: 0,
      books_unread: 1,
      last_read_at: null,
      first_book_id: null,
    }));
    mockOverview([{ ...overview[1], series_progress: series }]);
    render(<ReadingOverviewTab />);

    expect(await screen.findByText("Series 7")).toBeInTheDocument();
    expect(screen.queryByText("Series 8")).not.toBeInTheDocument();
    expect(screen.queryByRole("tab")).not.toBeInTheDocument();

    fireEvent.click(screen.getByText(/settings\.readingOverview\.showAll/));
    expect(screen.getByText("Series 9")).toBeInTheDocument();
    fireEvent.click(screen.getByText("settings.readingOverview.showLess"));
    expect(screen.queryByText("Series 9")).not.toBeInTheDocument();
  });

  it("shows an error when series books fail to load", async () => {
    mockOverview(overview);
    fetchMock.mockImplementation(async (url: string) =>
      url.includes("/series-books")
        ? { ok: false, json: async () => ({}) }
        : { ok: true, json: async () => overview },
    );
    render(<ReadingOverviewTab />);

    fireEvent.click(await screen.findByText("Serie1"));
    expect(await screen.findByText("Erreur de chargement")).toBeInTheDocument();
  });

  it("shows an empty state when there is no activity", async () => {
    mockOverview([]);
    render(<ReadingOverviewTab />);
    expect(await screen.findByText("settings.readingOverview.noActivity")).toBeInTheDocument();
  });

  it("shows an error when loading fails", async () => {
    fetchMock.mockRejectedValue(new Error("down"));
    render(<ReadingOverviewTab />);
    expect(await screen.findByText("Erreur de chargement")).toBeInTheDocument();
  });

  it("shows a loading indicator while fetching", async () => {
    fetchMock.mockReturnValue(new Promise(() => {}));
    render(<ReadingOverviewTab />);
    expect(screen.getByText("Chargement…")).toBeInTheDocument();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
  });
});
