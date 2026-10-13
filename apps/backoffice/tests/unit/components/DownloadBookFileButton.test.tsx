import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { DownloadBookFileButton } from "@/app/components/DownloadBookFileButton";

function setStandalone(value: boolean) {
  Object.defineProperty(navigator, "standalone", { value, configurable: true });
}

function setShare(share: unknown) {
  Object.defineProperty(navigator, "share", { value: share, configurable: true });
}

function setCanShare(canShare: unknown) {
  Object.defineProperty(navigator, "canShare", { value: canShare, configurable: true });
}

function fileResponse(headers: Record<string, string> = {}) {
  return new Response("bytes", { status: 200, headers });
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  Reflect.deleteProperty(navigator, "standalone");
  Reflect.deleteProperty(navigator, "share");
  Reflect.deleteProperty(navigator, "canShare");
});

describe("DownloadBookFileButton", () => {
  it("renders a download link to the book's original file", () => {
    render(<DownloadBookFileButton bookId="b1" />);

    const link = screen.getByRole("menuitem");
    expect(link.tagName).toBe("A");
    expect(link).toHaveAttribute("href", "/api/books/b1/file");
    expect(link).toHaveAttribute("download");
    expect(link).toHaveTextContent("bookDetail.downloadOriginal");
  });

  it("lets the native link work outside an installed iOS web app", () => {
    const share = vi.fn();
    setShare(share);
    setCanShare(vi.fn(() => true));
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);

    render(<DownloadBookFileButton bookId="b1" />);
    fireEvent.click(screen.getByRole("menuitem"));

    expect(fetchMock).not.toHaveBeenCalled();
    expect(share).not.toHaveBeenCalled();
  });

  it("shares the file through the native sheet in a standalone iOS web app", async () => {
    setStandalone(true);
    const share = vi.fn().mockResolvedValue(undefined);
    setShare(share);
    setCanShare(vi.fn(() => true));
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        fileResponse({ "content-disposition": 'attachment; filename="GON T06.cbz"' })
      )
    );

    render(<DownloadBookFileButton bookId="b1" />);
    fireEvent.click(screen.getByRole("menuitem"));

    await vi.waitFor(() => expect(share).toHaveBeenCalledTimes(1));
    const shared = share.mock.calls[0][0] as { files: File[] };
    expect(shared.files[0].name).toBe("GON T06.cbz");
  });

  it("falls back to opening the URL when sharing files is unsupported", async () => {
    setStandalone(true);
    setCanShare(vi.fn(() => false));
    const open = vi.spyOn(window, "open").mockImplementation(() => null);
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(fileResponse()));

    render(<DownloadBookFileButton bookId="b1" />);
    fireEvent.click(screen.getByRole("menuitem"));

    await vi.waitFor(() =>
      expect(open).toHaveBeenCalledWith("/api/books/b1/file", "_blank", "noopener")
    );
  });

  it("falls back to opening the URL when the file cannot be fetched", async () => {
    setStandalone(true);
    const open = vi.spyOn(window, "open").mockImplementation(() => null);
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("nope", { status: 500 })));

    render(<DownloadBookFileButton bookId="b1" />);
    fireEvent.click(screen.getByRole("menuitem"));

    await vi.waitFor(() => expect(open).toHaveBeenCalled());
  });

  it("stays quiet when the user dismisses the share sheet", async () => {
    setStandalone(true);
    const share = vi.fn().mockRejectedValue(Object.assign(new Error("abort"), { name: "AbortError" }));
    setShare(share);
    setCanShare(vi.fn(() => true));
    const open = vi.spyOn(window, "open").mockImplementation(() => null);
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        fileResponse({ "content-disposition": 'attachment; filename="book.cbz"' })
      )
    );

    render(<DownloadBookFileButton bookId="b1" />);
    fireEvent.click(screen.getByRole("menuitem"));

    await vi.waitFor(() => expect(share).toHaveBeenCalledTimes(1));
    expect(open).not.toHaveBeenCalled();
  });
});
