import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { MarkSeriesReadButton } from "@/app/components/MarkSeriesReadButton";

afterEach(() => {
  vi.unstubAllGlobals();
});

function stubFetch() {
  const fetchMock = vi.fn().mockResolvedValue({ ok: true });
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

describe("MarkSeriesReadButton", () => {
  it("marks the whole series as read when some books are unread", () => {
    const fetchMock = stubFetch();
    render(
      <MarkSeriesReadButton seriesId="series-1" seriesName="Series" bookCount={5} booksReadCount={2} />
    );

    fireEvent.click(screen.getByRole("button", { name: "markRead.markAllRead" }));

    expect(fetchMock).toHaveBeenCalledWith("/api/series/mark-read", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ series: "series-1", status: "read" }),
    });
  });

  it("marks the whole series as unread when every book is read", () => {
    const fetchMock = stubFetch();
    render(
      <MarkSeriesReadButton seriesId="series-1" seriesName="Series" bookCount={5} booksReadCount={5} />
    );

    expect(screen.getByRole("button", { name: "markRead.markUnread" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button"));

    expect(fetchMock).toHaveBeenCalledWith(
      "/api/series/mark-read",
      expect.objectContaining({ body: JSON.stringify({ series: "series-1", status: "unread" }) })
    );
  });

  it("treats a series without books as not fully read", () => {
    const fetchMock = stubFetch();
    render(
      <MarkSeriesReadButton seriesId="series-1" seriesName="Series" bookCount={0} booksReadCount={0} />
    );

    fireEvent.click(screen.getByRole("button", { name: "markRead.markAllRead" }));

    expect(fetchMock).toHaveBeenCalledWith(
      "/api/series/mark-read",
      expect.objectContaining({ body: JSON.stringify({ series: "series-1", status: "read" }) })
    );
  });

  it("uses the short labels in compact mode", () => {
    stubFetch();
    render(
      <MarkSeriesReadButton
        seriesId="series-1"
        seriesName="Series"
        bookCount={5}
        booksReadCount={1}
        compact
      />
    );

    expect(screen.getByRole("button", { name: "markRead.read" })).toBeInTheDocument();
  });
});
