import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { act } from "react";
import { hydrateRoot } from "react-dom/client";
import { renderToString } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { ReadingOverview } from "@/app/components/ReadingOverview";
import type { UserReadingOverviewDto } from "@/lib/api";

const fetchMock = vi.fn();

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
  window.localStorage.clear();
});

const overview: UserReadingOverviewDto[] = [
  {
    user_id: "u1",
    username: "Alice",
    books_read: 3,
    books_reading: 1,
    series_in_progress: 1,
    last_read_at: "2024-01-01",
    currently_reading: [
      {
        book_id: "b1",
        title: "Book1",
        series: "Series1",
        series_id: "s1",
        current_page: 5,
        page_count: 10,
        last_read_at: "2024-01-01",
      },
      {
        book_id: "b2",
        title: "Book2",
        series: null,
        series_id: null,
        current_page: 0,
        page_count: 0,
        last_read_at: null,
      },
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
      {
        book_id: "b9",
        title: "R1",
        series: "S",
        series_id: "s1",
        current_page: 0,
        page_count: 100,
        last_read_at: "2024-01-03",
      },
      {
        book_id: "b10",
        title: "R2",
        series: null,
        series_id: null,
        current_page: 0,
        page_count: 0,
        last_read_at: null,
      },
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
    recently_read: [],
    series_progress: [],
  },
];

const serieBooks = [
  { book_id: "b1", title: "V1", volume: 1, volume_type: "regular", status: "read", current_page: 0, page_count: 100, last_read_at: "2024-01-02" },
  { book_id: "b2", title: "V2", volume: null, volume_type: "hs", status: "reading", current_page: 10, page_count: 20, last_read_at: null },
  { book_id: "b3", title: "V3", volume: 2, volume_type: "oneshot", status: "unread", current_page: 0, page_count: 50, last_read_at: "x" },
  { book_id: "b4", title: "V4", volume: 3, volume_type: "integral", status: "read", current_page: 0, page_count: 0, last_read_at: null },
];

function mockSeriesBooks(payload: unknown = serieBooks) {
  fetchMock.mockResolvedValue({ ok: true, json: async () => payload });
}

describe("ReadingOverview", () => {
  it("renders the selected user's overview and expands series volumes", async () => {
    mockSeriesBooks();
    render(<ReadingOverview initialData={overview} />);

    expect(screen.getByText("Alice")).toBeInTheDocument();
    expect(screen.getByText("Bob")).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /Alice/ })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByText("Book1")).toBeInTheDocument();
    expect(screen.getByText("Book2")).toBeInTheDocument();
    expect(screen.getByText("Serie1")).toBeInTheDocument();
    expect(screen.getByText("Unclassified")).toBeInTheDocument();
    expect(screen.getByText("R1")).toBeInTheDocument();
    expect(screen.queryByText("Chargement…")).not.toBeInTheDocument();
    expect(fetchMock).not.toHaveBeenCalled();

    fireEvent.click(screen.getByText("Serie1"));
    expect(screen.getAllByTestId("volume-skeleton").length).toBeGreaterThan(0);
    expect(await screen.findByText("V1")).toBeInTheDocument();
    expect(fetchMock).toHaveBeenCalledWith("/api/reading-overview/u1/series-books?series_id=s1");
    expect(screen.getByText("T1")).toBeInTheDocument();
    expect(screen.getByText("OS")).toBeInTheDocument();
    expect(screen.getByText("Int 3")).toBeInTheDocument();
    expect(screen.getByText(/10\/20/)).toBeInTheDocument();
    expect(fetchMock).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByText("Serie1"));
    expect(screen.queryByText("V1")).not.toBeInTheDocument();

    // Re-expanding reuses the already-loaded books.
    fireEvent.click(screen.getByText("Serie1"));
    expect(screen.getByText("V1")).toBeInTheDocument();
    expect(fetchMock).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByText("Unclassified"));
    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith("/api/reading-overview/u1/series-books"),
    );

    fireEvent.click(screen.getByRole("tab", { name: /Bob/ }));
    expect(await screen.findByText("settings.readingOverview.noSeries")).toBeInTheDocument();
    expect(screen.queryByText("Book1")).not.toBeInTheDocument();
  });

  it("switches the series statuses between list and grid views", async () => {
    mockSeriesBooks();
    render(<ReadingOverview initialData={overview} />);

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
    expect(window.localStorage.getItem("stripstream:reading-overview:series-view")).toBe("grid");

    // Cards stay expandable to reveal the series volumes.
    fireEvent.click(screen.getByText("Serie1"));
    expect(await screen.findByText("V1")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "settings.readingOverview.viewList" }));
    expect(screen.getByTestId("series-list")).toBeInTheDocument();
    expect(screen.queryByTestId("series-grid")).not.toBeInTheDocument();
    expect(window.localStorage.getItem("stripstream:reading-overview:series-view")).toBe("list");
  });

  it("restores the persisted series view from localStorage", async () => {
    window.localStorage.setItem("stripstream:reading-overview:series-view", "grid");
    render(<ReadingOverview initialData={overview} />);

    expect(await screen.findByTestId("series-grid")).toBeInTheDocument();
    expect(screen.queryByTestId("series-list")).not.toBeInTheDocument();
  });

  it("hydrates the server-rendered list then applies the persisted grid view", async () => {
    window.localStorage.setItem("stripstream:reading-overview:series-view", "grid");

    const html = renderToString(<ReadingOverview initialData={overview} />);
    expect(html).toContain('data-testid="series-list"');
    expect(html).not.toContain('data-testid="series-grid"');

    const container = document.createElement("div");
    container.innerHTML = html;
    document.body.appendChild(container);

    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
    const actEnvironment = globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean };
    const previousActEnvironment = actEnvironment.IS_REACT_ACT_ENVIRONMENT;
    actEnvironment.IS_REACT_ACT_ENVIRONMENT = true;
    let root: ReturnType<typeof hydrateRoot> | null = null;

    try {
      await act(async () => {
        root = hydrateRoot(container, <ReadingOverview initialData={overview} />);
      });

      await waitFor(() => {
        expect(container.querySelector('[data-testid="series-grid"]')).not.toBeNull();
      });
      expect(consoleError).not.toHaveBeenCalled();
    } finally {
      await act(async () => {
        root?.unmount();
      });
      container.remove();
      actEnvironment.IS_REACT_ACT_ENVIRONMENT = previousActEnvironment;
      consoleError.mockRestore();
    }
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
    render(
      <ReadingOverview initialData={[{ ...overview[1], series_progress: series }]} />,
    );

    expect(screen.getByText("Series 7")).toBeInTheDocument();
    expect(screen.queryByText("Series 8")).not.toBeInTheDocument();
    expect(screen.queryByRole("tab")).not.toBeInTheDocument();

    fireEvent.click(screen.getByText(/settings\.readingOverview\.showAll/));
    expect(screen.getByText("Series 9")).toBeInTheDocument();
    fireEvent.click(screen.getByText("settings.readingOverview.showLess"));
    expect(screen.queryByText("Series 9")).not.toBeInTheDocument();
  });

  it("dims unread volumes and keeps read/reading markers", async () => {
    mockSeriesBooks();
    render(<ReadingOverview initialData={overview} />);

    fireEvent.click(screen.getByText("Serie1"));
    expect(await screen.findByText("V3")).toBeInTheDocument();

    expect(screen.getByText("V3").closest("a")?.querySelector(".opacity-45")).not.toBeNull();
    expect(screen.getByText("V1").closest("a")?.querySelector(".opacity-45")).toBeNull();
    expect(screen.getAllByText("V2")[0].closest("a")?.textContent).toContain("10/20");
  });

  it("applies the shared reveal and stagger animations", () => {
    const { container } = render(<ReadingOverview initialData={overview} />);

    expect(container.querySelectorAll("[data-reveal]")).toHaveLength(4);
    expect(container.querySelector(".stagger-children")).not.toBeNull();
    expect(container.querySelector(".stagger-deal")).not.toBeNull();
  });

  it("shows an error when series books fail to load", async () => {
    fetchMock.mockResolvedValue({ ok: false, json: async () => ({}) });
    render(<ReadingOverview initialData={overview} />);

    fireEvent.click(screen.getByText("Serie1"));
    expect(await screen.findByText("settings.readingOverview.loadError")).toBeInTheDocument();
  });

  it("shows an empty state when there is no activity", () => {
    render(<ReadingOverview initialData={[]} />);
    expect(screen.getByText("settings.readingOverview.noActivity")).toBeInTheDocument();
  });
});
