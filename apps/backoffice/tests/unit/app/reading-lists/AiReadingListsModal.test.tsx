import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { AiReadingListsModal } from "@/app/(app)/reading-lists/AiReadingListsModal";

const fetchMock = vi.fn();

const seriesOne = {
  id: "series-1",
  name: "First Series",
  cover_url: null,
  first_book_id: null,
  first_book_updated_at: null,
  library_name: "Main",
};

const seriesTwo = { ...seriesOne, id: "series-2", name: "Second Series" };

function jsonResponse(body: unknown, ok = true, status = ok ? 200 : 500): Response {
  return { ok, status, json: async () => body } as Response;
}

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

describe("AiReadingListsModal", () => {
  it("loads proposals and adds every selected series to the existing list", async () => {
    const onApplied = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();
    fetchMock.mockImplementation((url: string, init?: RequestInit) => {
      if (url === "/api/reading-lists/ai-suggest") {
        return Promise.resolve(
          jsonResponse({
            additions: [
              {
                list_id: "list-1",
                list_name: "Favourites",
                reason: "Same universe",
                series: [seriesOne, seriesTwo],
              },
            ],
            new_lists: [],
          })
        );
      }
      if (url === "/api/reading-lists/list-1/series" && init?.method === "POST") {
        return Promise.resolve(jsonResponse(null));
      }
      return Promise.resolve(jsonResponse({}, false));
    });

    render(<AiReadingListsModal isOpen onClose={onClose} onApplied={onApplied} />);

    expect(await screen.findByText("Favourites")).toBeInTheDocument();
    expect(screen.getByText("Same universe")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: /readingLists\.aiApply/ }));

    await waitFor(() => expect(onApplied).toHaveBeenCalled());
    expect(onClose).toHaveBeenCalled();
    const seriesCalls = fetchMock.mock.calls.filter(
      ([url]) => url === "/api/reading-lists/list-1/series"
    );
    expect(seriesCalls).toHaveLength(2);
    expect(seriesCalls[0][1]).toMatchObject({
      method: "POST",
      body: JSON.stringify({ series_id: "series-1" }),
    });
    expect(seriesCalls[1][1]).toMatchObject({
      method: "POST",
      body: JSON.stringify({ series_id: "series-2" }),
    });
  });

  it("does not add a series that was deselected", async () => {
    const onApplied = vi.fn().mockResolvedValue(undefined);
    fetchMock.mockImplementation((url: string, init?: RequestInit) => {
      if (url === "/api/reading-lists/ai-suggest") {
        return Promise.resolve(
          jsonResponse({
            additions: [
              {
                list_id: "list-1",
                list_name: "Favourites",
                reason: "Same universe",
                series: [seriesOne, seriesTwo],
              },
            ],
            new_lists: [],
          })
        );
      }
      if (url === "/api/reading-lists/list-1/series" && init?.method === "POST") {
        return Promise.resolve(jsonResponse(null));
      }
      return Promise.resolve(jsonResponse({}, false));
    });

    render(<AiReadingListsModal isOpen onClose={vi.fn()} onApplied={onApplied} />);
    await screen.findByText("Favourites");

    await userEvent.click(screen.getByRole("checkbox", { name: /Second Series/ }));
    await userEvent.click(screen.getByRole("button", { name: /readingLists\.aiApply/ }));

    await waitFor(() => expect(onApplied).toHaveBeenCalled());
    const seriesCalls = fetchMock.mock.calls.filter(
      ([url]) => url === "/api/reading-lists/list-1/series"
    );
    expect(seriesCalls).toHaveLength(1);
    expect(seriesCalls[0][1]?.body).toBe(JSON.stringify({ series_id: "series-1" }));
  });

  it("creates a new list with the edited name and description", async () => {
    const onApplied = vi.fn().mockResolvedValue(undefined);
    fetchMock.mockImplementation((url: string, init?: RequestInit) => {
      if (url === "/api/reading-lists/ai-suggest") {
        return Promise.resolve(
          jsonResponse({
            additions: [],
            new_lists: [
              {
                name: "Space Opera",
                description: "Far away adventures.",
                reason: "Shared setting",
                series: [seriesOne, seriesTwo],
              },
            ],
          })
        );
      }
      if (url === "/api/reading-lists" && init?.method === "POST") {
        return Promise.resolve(jsonResponse({ id: "new-list" }));
      }
      if (url === "/api/reading-lists/new-list/series" && init?.method === "POST") {
        return Promise.resolve(jsonResponse(null));
      }
      return Promise.resolve(jsonResponse({}, false));
    });

    render(<AiReadingListsModal isOpen onClose={vi.fn()} onApplied={onApplied} />);
    const name = await screen.findByDisplayValue("Space Opera");
    const description = screen.getByDisplayValue("Far away adventures.");

    fireEvent.change(name, { target: { value: "Space Saga" } });
    fireEvent.change(description, { target: { value: "Sagas set in space." } });

    await userEvent.click(screen.getByRole("button", { name: /readingLists\.aiApply/ }));

    await waitFor(() => expect(onApplied).toHaveBeenCalled());
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/reading-lists",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({ name: "Space Saga", description: "Sagas set in space." }),
      })
    );
    const seriesCalls = fetchMock.mock.calls.filter(
      ([url]) => url === "/api/reading-lists/new-list/series"
    );
    expect(seriesCalls).toHaveLength(2);
  });

  it("shows the empty state when nothing is proposed", async () => {
    fetchMock.mockResolvedValue(jsonResponse({ additions: [], new_lists: [] }));

    render(<AiReadingListsModal isOpen onClose={vi.fn()} onApplied={vi.fn()} />);

    expect(await screen.findByText("readingLists.aiEmpty")).toBeInTheDocument();
  });

  it("shows an error and can retry", async () => {
    fetchMock
      .mockResolvedValueOnce(jsonResponse({ error: "AI disabled" }, false))
      .mockResolvedValueOnce(jsonResponse({ additions: [], new_lists: [] }));

    render(<AiReadingListsModal isOpen onClose={vi.fn()} onApplied={vi.fn()} />);

    expect(await screen.findByText("AI disabled")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "readingLists.aiRetry" }));
    expect(await screen.findByText("readingLists.aiEmpty")).toBeInTheDocument();
  });

  it("shows a localised error when the AI is disabled", async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({ error: "AI reading list suggestions are disabled" }, false, 422)
    );

    render(<AiReadingListsModal isOpen onClose={vi.fn()} onApplied={vi.fn()} />);

    expect(await screen.findByText("readingLists.aiError")).toBeInTheDocument();
  });

  it("refuses to create a new list with fewer than two series", async () => {
    const onApplied = vi.fn();
    fetchMock.mockImplementation((url: string) => {
      if (url === "/api/reading-lists/ai-suggest") {
        return Promise.resolve(
          jsonResponse({
            additions: [],
            new_lists: [
              {
                name: "Space Opera",
                description: "Far away adventures.",
                reason: "Shared setting",
                series: [seriesOne, seriesTwo],
              },
            ],
          })
        );
      }
      return Promise.resolve(jsonResponse(null));
    });

    render(<AiReadingListsModal isOpen onClose={vi.fn()} onApplied={onApplied} />);
    await screen.findByDisplayValue("Space Opera");

    await userEvent.click(screen.getByRole("checkbox", { name: /Second Series/ }));

    expect(screen.getByText("readingLists.aiNewListInvalid")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /readingLists\.aiApply/ })).toBeDisabled();
    expect(fetchMock).not.toHaveBeenCalledWith("/api/reading-lists", expect.anything());
  });

  it("refuses to create a new list without a description", async () => {
    fetchMock.mockImplementation((url: string) => {
      if (url === "/api/reading-lists/ai-suggest") {
        return Promise.resolve(
          jsonResponse({
            additions: [],
            new_lists: [
              {
                name: "Space Opera",
                description: "Far away adventures.",
                reason: "Shared setting",
                series: [seriesOne, seriesTwo],
              },
            ],
          })
        );
      }
      return Promise.resolve(jsonResponse(null));
    });

    render(<AiReadingListsModal isOpen onClose={vi.fn()} onApplied={vi.fn()} />);
    const description = await screen.findByDisplayValue("Far away adventures.");

    fireEvent.change(description, { target: { value: "   " } });

    expect(screen.getByText("readingLists.aiNewListInvalid")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /readingLists\.aiApply/ })).toBeDisabled();
  });

  it("removes a new list when none of its series could be added", async () => {
    const onApplied = vi.fn();
    const onClose = vi.fn();
    fetchMock.mockImplementation((url: string, init?: RequestInit) => {
      if (url === "/api/reading-lists/ai-suggest") {
        return Promise.resolve(
          jsonResponse({
            additions: [],
            new_lists: [
              {
                name: "Space Opera",
                description: "Far away adventures.",
                reason: "Shared setting",
                series: [seriesOne, seriesTwo],
              },
            ],
          })
        );
      }
      if (url === "/api/reading-lists" && init?.method === "POST") {
        return Promise.resolve(jsonResponse({ id: "new-list" }));
      }
      if (url === "/api/reading-lists/new-list/series") {
        return Promise.resolve(jsonResponse({}, false));
      }
      if (url === "/api/reading-lists/new-list" && init?.method === "DELETE") {
        return Promise.resolve(jsonResponse(null));
      }
      return Promise.resolve(jsonResponse({}, false));
    });

    render(<AiReadingListsModal isOpen onClose={onClose} onApplied={onApplied} />);
    await screen.findByDisplayValue("Space Opera");

    await userEvent.click(screen.getByRole("button", { name: /readingLists\.aiApply/ }));

    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith("/api/reading-lists/new-list", {
        method: "DELETE",
      })
    );
    expect(onApplied).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
  });
});
