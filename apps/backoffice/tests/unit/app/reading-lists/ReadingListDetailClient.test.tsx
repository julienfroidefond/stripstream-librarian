import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const refresh = vi.hoisted(() => vi.fn());
const toast = vi.hoisted(() => vi.fn());

vi.mock("next/navigation", () => ({
  useRouter: () => ({
    push: vi.fn(),
    replace: vi.fn(),
    refresh,
    back: vi.fn(),
    prefetch: vi.fn(),
  }),
}));

vi.mock("@/app/components/ui/Toast", () => ({ toast }));

import { ReadingListDetailClient } from "@/app/(app)/reading-lists/[id]/ReadingListDetailClient";
import type { ReadingListDetailDto, ReadingListSeriesDto } from "@/lib/api";

function item(id: string, name: string, position: number): ReadingListSeriesDto {
  return {
    id,
    name,
    cover_url: null,
    first_book_id: null,
    first_book_updated_at: null,
    provider: null,
    external_id: null,
    external_url: null,
    library_id: "lib-1",
    library_name: "Main library",
    position,
    book_count: 2,
    books_read_count: 0,
    is_empty: false,
  };
}

const list: ReadingListDetailDto = {
  id: "list-1",
  name: "Favourites",
  description: null,
  items: [item("s1", "Alpha", 0), item("s2", "Beta", 1)],
  created_at: "2026-10-20T00:00:00Z",
  updated_at: "2026-10-20T00:00:00Z",
};

function json(body: unknown, status = 200): Response {
  return {
    ok: status < 400,
    status,
    json: async () => body,
    clone() {
      return this;
    },
    text: async () => JSON.stringify(body),
  } as unknown as Response;
}

const fetchMock = vi.fn();

beforeEach(() => {
  refresh.mockClear();
  toast.mockClear();
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("ReadingListDetailClient", () => {
  it("adds a series once the server confirmed it and shows the re-read row", async () => {
    fetchMock.mockImplementation((url: string, init?: RequestInit) => {
      if (url.startsWith("/api/series")) {
        return Promise.resolve(
          json({
            items: [{ series_id: "s3", name: "Gamma", cover_url: null, first_book_id: null, metadata_provider: null }],
            total: 1,
          })
        );
      }
      if (url === "/api/reading-lists/list-1/series" && init?.method === "POST") {
        return Promise.resolve(json(undefined, 204));
      }
      if (url === "/api/reading-lists/list-1") {
        return Promise.resolve(
          json({
            ...list,
            items: [...list.items, { ...item("s3", "Gamma", 2), library_name: "Second library" }],
          })
        );
      }
      return Promise.resolve(json({ error: "unexpected" }, 500));
    });

    render(<ReadingListDetailClient list={list} />);

    await userEvent.click(screen.getByRole("button", { name: /readingLists\.addSeries/ }));
    const modal = screen.getByTestId("modal-panel");
    fireEvent.change(within(modal).getByPlaceholderText("readingLists.searchSeries"), {
      target: { value: "Gam" },
    });
    await userEvent.click(await within(modal).findByRole("button", { name: /Gamma/ }));

    // The new row comes from the server read, not from a local guess.
    expect(await screen.findByText("Second library")).toBeInTheDocument();
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/reading-lists/list-1",
      expect.objectContaining({ method: "GET" })
    );
    expect(refresh).toHaveBeenCalledTimes(1);
    expect(toast).not.toHaveBeenCalled();
  });

  it("moves a series down optimistically and rolls back when the server refuses", async () => {
    let rejectReorder!: () => void;
    fetchMock.mockImplementation((url: string) => {
      if (url === "/api/reading-lists/list-1/series/reorder") {
        return new Promise<Response>((resolve) => {
          rejectReorder = () => resolve(json({ error: "reorder refused" }, 400));
        });
      }
      return Promise.resolve(json({ error: "unexpected" }, 500));
    });

    render(<ReadingListDetailClient list={list} />);

    const [moveDown] = screen.getAllByTitle("readingLists.moveDown");
    await userEvent.click(moveDown);

    const order = () => screen.getAllByText(/^(Alpha|Beta)$/).map((node) => node.textContent);
    expect(order()).toEqual(["Beta", "Alpha"]);

    rejectReorder();
    await waitFor(() => expect(order()).toEqual(["Alpha", "Beta"]));
    expect(toast).toHaveBeenCalledWith("reorder refused", "error");
    expect(refresh).not.toHaveBeenCalled();
    const [, request] = fetchMock.mock.calls[0];
    expect(request).toMatchObject({
      method: "PUT",
      body: JSON.stringify({ series_ids: ["s2", "s1"] }),
    });
  });

  it("keeps a series whose removal fails and reports it", async () => {
    fetchMock.mockResolvedValue(json({ error: "nope" }, 500));

    render(<ReadingListDetailClient list={list} />);
    const [remove] = screen.getAllByTitle("readingLists.removeSeries");
    await userEvent.click(remove);

    await waitFor(() => expect(toast).toHaveBeenCalledWith("nope", "error"));
    expect(screen.getByText("Alpha")).toBeInTheDocument();
    expect(refresh).not.toHaveBeenCalled();
  });

  it("renames the list from the server answer", async () => {
    fetchMock.mockResolvedValue(
      json({ ...list, name: "Best of", description: "Top picks", series_count: 2 })
    );

    render(<ReadingListDetailClient list={list} />);
    await userEvent.click(screen.getByRole("button", { name: "readingLists.editTitle" }));
    const nameInput = screen.getByDisplayValue("Favourites");
    fireEvent.change(nameInput, { target: { value: "Best of" } });
    await userEvent.click(screen.getByRole("button", { name: "common.save" }));

    expect(await screen.findByRole("heading", { name: "Best of" })).toBeInTheDocument();
    expect(screen.getByText("Top picks")).toBeInTheDocument();
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/reading-lists/list-1",
      expect.objectContaining({ method: "PATCH" })
    );
    expect(refresh).toHaveBeenCalledTimes(1);
  });
});
