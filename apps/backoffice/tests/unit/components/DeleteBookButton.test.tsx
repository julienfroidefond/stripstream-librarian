import { fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const routerMock = vi.hoisted(() => ({
  push: vi.fn(),
  refresh: vi.fn(),
  replace: vi.fn(),
  back: vi.fn(),
  prefetch: vi.fn(),
}));
vi.mock("next/navigation", () => ({ useRouter: () => routerMock }));

import { DeleteBookButton } from "@/app/components/DeleteBookButton";

afterEach(() => {
  vi.unstubAllGlobals();
});

function confirmDelete({ bookId = "b1", seriesId }: { bookId?: string; seriesId?: string } = {}) {
  render(
    <DeleteBookButton bookId={bookId} seriesId={seriesId}>
      {(open) => (
        <button type="button" onClick={open}>
          open
        </button>
      )}
    </DeleteBookButton>
  );

  fireEvent.click(screen.getByRole("button", { name: "open" }));
  const panel = screen.getByTestId("modal-panel");
  fireEvent.click(within(panel).getByRole("button", { name: "bookDetail.delete" }));
}

describe("DeleteBookButton", () => {
  it("deletes the book and redirects to its series page", async () => {
    const fetchMock = vi.fn().mockResolvedValue({ ok: true });
    vi.stubGlobal("fetch", fetchMock);

    confirmDelete({ seriesId: "s1" });

    expect(fetchMock).toHaveBeenCalledWith("/api/books/b1", { method: "DELETE" });
    await vi.waitFor(() => expect(routerMock.push).toHaveBeenCalledWith("/series/s1"));
    expect(routerMock.refresh).toHaveBeenCalled();
  });

  it("falls back to the series list without a seriesId", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: true }));

    confirmDelete();

    await vi.waitFor(() => expect(routerMock.push).toHaveBeenCalledWith("/series"));
  });
});
