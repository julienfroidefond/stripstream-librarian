import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
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
        books_total: 3,
        books_read: 2,
        books_reading: 1,
        books_unread: 0,
        last_read_at: "2024-01-02",
        books: [
          { book_id: "b1", title: "V1", volume: 1, volume_type: "tankoubon", status: "read", current_page: 0, page_count: 100, last_read_at: "2024-01-02" },
          { book_id: "b2", title: "V2", volume: null, volume_type: "hs", status: "reading", current_page: 10, page_count: 20, last_read_at: null },
          { book_id: "b3", title: "V3", volume: 2, volume_type: "oneshot", status: "unread", current_page: 0, page_count: 50, last_read_at: "x" },
          { book_id: "b4", title: "V4", volume: 3, volume_type: "integral", status: "read", current_page: 0, page_count: 0, last_read_at: null },
          { book_id: "b5", title: "V5", volume: null, volume_type: "other", status: "unread", current_page: 0, page_count: 0, last_read_at: null },
        ],
      },
      {
        series_id: null,
        series_name: "Unclassified",
        books_total: 0,
        books_read: 0,
        books_reading: 0,
        books_unread: 0,
        last_read_at: null,
        books: [],
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

function mockOverview(payload: unknown) {
  fetchMock.mockResolvedValue({ json: async () => payload });
}

describe("ReadingOverviewTab", () => {
  it("renders the overview and expands series books", async () => {
    mockOverview(overview);
    render(<ReadingOverviewTab />);

    expect(await screen.findByText("Alice")).toBeInTheDocument();
    expect(screen.getByText("Book1")).toBeInTheDocument();
    expect(screen.getByText("Book2")).toBeInTheDocument();
    expect(screen.getByText("Bob")).toBeInTheDocument();

    fireEvent.click(screen.getAllByText(/settings\.readingOverview\.seriesStatuses/)[0]);
    expect(await screen.findByText("Serie1")).toBeInTheDocument();
    expect(screen.getByText("Unclassified")).toBeInTheDocument();

    fireEvent.click(screen.getByText("Serie1"));
    expect(await screen.findByText("V1")).toBeInTheDocument();
    expect(screen.getByText("T1")).toBeInTheDocument();
    expect(screen.getByText("HS")).toBeInTheDocument();
    expect(screen.getByText("OS")).toBeInTheDocument();
    expect(screen.getByText("Int 3")).toBeInTheDocument();
    expect(screen.getByText("10/20")).toBeInTheDocument();

    fireEvent.click(screen.getByText("Serie1"));
    expect(screen.queryByText("V1")).not.toBeInTheDocument();

    fireEvent.click(screen.getAllByText(/settings\.readingOverview\.seriesStatuses/)[1]);
    expect(await screen.findByText("settings.readingOverview.noSeries")).toBeInTheDocument();
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
