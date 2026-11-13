import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const refresh = vi.hoisted(() => vi.fn());

vi.mock("next/navigation", () => ({
  useRouter: () => ({
    push: vi.fn(),
    replace: vi.fn(),
    refresh,
    back: vi.fn(),
    prefetch: vi.fn(),
  }),
}));

// Stand-in for the AI modal: it writes through the page's shared store, like
// the real one does with its approved proposals.
vi.mock("@/app/(app)/reading-lists/AiReadingListsModal", async () => {
  const { useReadingLists } = await import("@/app/components/ReadingListsProvider");
  function AiReadingListsModal({ isOpen }: { isOpen: boolean }) {
    const readingLists = useReadingLists();
    return isOpen ? (
      <button
        type="button"
        onClick={() =>
          void readingLists.runAll([{ type: "add", listId: "list-1", seriesId: "series-1" }])
        }
      >
        apply-ai
      </button>
    ) : null;
  }
  return { AiReadingListsModal };
});

import { ReadingListsClient } from "@/app/(app)/reading-lists/ReadingListsClient";

const createdList = {
  id: "list-1",
  name: "Favourites",
  description: null,
  series_count: 1,
  book_count: 1,
  books_read_count: 0,
  is_incomplete: true,
  preview_covers: [],
  created_at: "2026-10-20T00:00:00Z",
  updated_at: "2026-10-20T00:00:00Z",
};

const fetchMock = vi.fn();

beforeEach(() => {
  refresh.mockClear();
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("ReadingListsClient", () => {
  it("reconciles the overview from the server after AI proposals are applied", async () => {
    fetchMock.mockImplementation((_url: string, init?: RequestInit) =>
      Promise.resolve({
        ok: true,
        status: init?.method === "POST" ? 204 : 200,
        json: async () => [{ ...createdList, name: "Favourites (2)", series_count: 2 }],
      } as Response)
    );

    render(<ReadingListsClient initialLists={[createdList]} hasActiveUser={false} />);

    await userEvent.click(
      screen.getByRole("button", { name: "readingLists.aiSuggest" })
    );
    await userEvent.click(screen.getByRole("button", { name: "apply-ai" }));

    await waitFor(() => expect(refresh).toHaveBeenCalledTimes(1));
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/reading-lists",
      expect.objectContaining({ method: "GET" })
    );
    expect(await screen.findByText("Favourites (2)")).toBeInTheDocument();
  });

  it("keeps a list whose deletion fails, reports it and does not refresh", async () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    fetchMock.mockResolvedValue({
      ok: false,
      status: 500,
      json: async () => ({ error: "cannot delete" }),
    } as Response);

    render(
      <ReadingListsClient initialLists={[createdList]} hasActiveUser={false} />
    );
    await userEvent.click(screen.getByTitle("common.delete"));

    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/reading-lists/list-1",
        expect.objectContaining({ method: "DELETE" })
      )
    );
    await waitFor(() => expect(screen.getByTitle("common.delete")).not.toBeDisabled());
    expect(screen.getByText("Favourites")).toBeInTheDocument();
    expect(refresh).not.toHaveBeenCalled();
    confirm.mockRestore();
  });

  it("refreshes the router cache after creating a list", async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      json: async () => createdList,
    } as Response);

    render(<ReadingListsClient initialLists={[]} hasActiveUser={false} />);

    const [openCreate] = screen.getAllByRole("button", {
      name: "readingLists.create",
    });
    await userEvent.click(openCreate);
    await userEvent.type(
      screen.getByPlaceholderText("readingLists.namePlaceholder"),
      "Favourites"
    );
    await userEvent.click(
      within(screen.getByTestId("modal-panel")).getByRole("button", {
        name: "readingLists.create",
      })
    );

    await waitFor(() => expect(refresh).toHaveBeenCalledTimes(1));
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/reading-lists",
      expect.objectContaining({ method: "POST" })
    );
  });

  it("refreshes the router cache after deleting a list", async () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    fetchMock.mockResolvedValue({ ok: true } as Response);

    render(
      <ReadingListsClient initialLists={[createdList]} hasActiveUser={false} />
    );

    await userEvent.click(
      screen.getByTitle("common.delete")
    );

    await waitFor(() => expect(refresh).toHaveBeenCalledTimes(1));
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/reading-lists/list-1",
      expect.objectContaining({ method: "DELETE" })
    );
    confirm.mockRestore();
  });
});
