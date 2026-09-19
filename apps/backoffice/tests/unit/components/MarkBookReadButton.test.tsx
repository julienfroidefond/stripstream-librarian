import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { MarkBookReadButton } from "@/app/components/MarkBookReadButton";

afterEach(() => {
  vi.unstubAllGlobals();
});

function stubFetch() {
  const fetchMock = vi.fn().mockResolvedValue({ ok: true });
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

describe("MarkBookReadButton", () => {
  it("offers to mark an unread book as read", () => {
    stubFetch();
    render(<MarkBookReadButton bookId="book-1" currentStatus="unread" />);

    expect(screen.getByRole("button", { name: "markRead.markAsRead" })).toBeInTheDocument();
  });

  it("offers to mark a read book as unread", () => {
    stubFetch();
    render(<MarkBookReadButton bookId="book-1" currentStatus="read" />);

    expect(screen.getByRole("button", { name: "markRead.markUnread" })).toBeInTheDocument();
  });

  it("uses the short labels in compact mode", () => {
    stubFetch();
    render(<MarkBookReadButton bookId="book-1" currentStatus="unread" compact />);

    expect(screen.getByRole("button", { name: "markRead.read" })).toBeInTheDocument();
  });

  it("patches the book progress to read", async () => {
    const fetchMock = stubFetch();
    render(<MarkBookReadButton bookId="book-1" currentStatus="unread" />);

    fireEvent.click(screen.getByRole("button"));
    expect(screen.getByRole("button")).toBeDisabled();

    expect(fetchMock).toHaveBeenCalledWith("/api/books/book-1/progress", {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ status: "read" }),
    });
    await vi.waitFor(() => expect(screen.getByRole("button")).not.toBeDisabled());
  });

  it("patches the book progress back to unread", () => {
    const fetchMock = stubFetch();
    render(<MarkBookReadButton bookId="book-1" currentStatus="read" />);

    fireEvent.click(screen.getByRole("button"));

    expect(fetchMock).toHaveBeenCalledWith(
      "/api/books/book-1/progress",
      expect.objectContaining({ body: JSON.stringify({ status: "unread" }) })
    );
  });
});
