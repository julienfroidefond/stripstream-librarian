import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  getReadingOverview: vi.fn(),
  getBookCoverUrl: vi.fn((bookId: string) => `/covers/${bookId}.webp`),
}));

vi.mock("@/lib/api", () => api);
vi.mock("@/lib/i18n/server", () => ({
  getServerTranslations: async () => ({ t: (key: string) => key, locale: "en", setLocale: vi.fn() }),
}));

import ProgressPage from "@/app/(app)/progress/page";

const overview = [
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
    ],
    series_progress: [],
    recently_read: [],
  },
];

beforeEach(() => {
  api.getReadingOverview.mockReset();
  api.getBookCoverUrl.mockClear();
  window.localStorage.clear();
});

describe("ProgressPage", () => {
  it("renders the server-fetched overview in the initial markup", async () => {
    api.getReadingOverview.mockResolvedValue(overview);

    render(await ProgressPage());

    expect(api.getReadingOverview).toHaveBeenCalledTimes(1);
    expect(screen.getByText("nav.progress")).toBeInTheDocument();
    expect(screen.getByText("Alice")).toBeInTheDocument();
    expect(screen.getAllByText("Book1").length).toBeGreaterThan(0);
    expect(screen.getByText("Series1")).toBeInTheDocument();
    expect(screen.queryByText("Chargement…")).not.toBeInTheDocument();
  });

  it("renders the empty state when there is no reading activity", async () => {
    api.getReadingOverview.mockResolvedValue([]);

    render(await ProgressPage());

    expect(screen.getByText("settings.readingOverview.noActivity")).toBeInTheDocument();
  });

  it("propagates server fetch failures to the error boundary", async () => {
    api.getReadingOverview.mockRejectedValue(new Error("api down"));

    await expect(ProgressPage()).rejects.toThrow("api down");
  });
});
