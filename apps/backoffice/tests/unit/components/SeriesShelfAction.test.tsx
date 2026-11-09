import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { refreshMock } = vi.hoisted(() => ({
  refreshMock: vi.fn(),
}));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ refresh: refreshMock }),
}));

import { SeriesShelfAction } from "@/app/components/SeriesShelfAction";

const fetchMock = vi.fn();

beforeEach(() => {
  fetchMock.mockReset();
  refreshMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("SeriesShelfAction", () => {
  it("puts the series aside and refreshes the current page", async () => {
    fetchMock.mockResolvedValue({ ok: true, json: async () => null });
    render(<SeriesShelfAction seriesId="s1" initialIsShelved={false} />);

    fireEvent.click(screen.getByRole("menuitem", { name: "seriesShelf.shelve" }));

    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith("/api/series/s1/shelf", { method: "PUT" })
    );
    await waitFor(() => expect(refreshMock).toHaveBeenCalledTimes(1));
    expect(screen.getByRole("menuitem", { name: "seriesShelf.resume" })).toBeInTheDocument();
  });

  it("resumes a shelved series", async () => {
    fetchMock.mockResolvedValue({ ok: true, json: async () => null });
    render(<SeriesShelfAction seriesId="s1" initialIsShelved />);

    fireEvent.click(screen.getByRole("menuitem", { name: "seriesShelf.resume" }));

    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith("/api/series/s1/shelf", { method: "DELETE" })
    );
    await waitFor(() => expect(refreshMock).toHaveBeenCalledTimes(1));
    expect(screen.getByRole("menuitem", { name: "seriesShelf.shelve" })).toBeInTheDocument();
  });

  it("keeps the current state and does not refresh on error", async () => {
    fetchMock.mockResolvedValue({ ok: false, json: async () => ({ error: "API down" }) });
    render(<SeriesShelfAction seriesId="s1" initialIsShelved={false} />);

    fireEvent.click(screen.getByRole("menuitem", { name: "seriesShelf.shelve" }));

    await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(1));
    await waitFor(() =>
      expect(screen.getByRole("menuitem", { name: "seriesShelf.shelve" })).toBeInTheDocument()
    );
    expect(refreshMock).not.toHaveBeenCalled();
  });
});
