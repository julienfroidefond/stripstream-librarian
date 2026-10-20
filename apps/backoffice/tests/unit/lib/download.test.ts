import { afterEach, describe, expect, it, vi } from "vitest";

import {
  fetchSharedFile,
  filenameFromContentDisposition,
  isIosStandalone,
  isTelegramDownloadRetryable,
  showsDownloadProgress,
} from "@/lib/download";

afterEach(() => {
  vi.unstubAllGlobals();
  Reflect.deleteProperty(navigator, "standalone");
});

describe("isIosStandalone", () => {
  it("is false in a regular browser", () => {
    expect(isIosStandalone()).toBe(false);
  });

  it("is true when iOS reports standalone mode", () => {
    Object.defineProperty(navigator, "standalone", { value: true, configurable: true });
    expect(isIosStandalone()).toBe(true);
  });
});

describe("filenameFromContentDisposition", () => {
  it("prefers the RFC 5987 encoded filename", () => {
    expect(
      filenameFromContentDisposition(
        "attachment; filename=\"Ast_rix le Gaulois.cbz\"; filename*=UTF-8''Ast%C3%A9rix%20le%20Gaulois.cbz"
      )
    ).toBe("Astérix le Gaulois.cbz");
  });

  it("falls back to the plain ASCII filename", () => {
    expect(filenameFromContentDisposition('attachment; filename="book.cbz"')).toBe("book.cbz");
  });

  it("returns null for missing or malformed headers", () => {
    expect(filenameFromContentDisposition(null)).toBeNull();
    expect(filenameFromContentDisposition("attachment")).toBeNull();
    expect(filenameFromContentDisposition("attachment; filename*=UTF-8''%E9")).toBeNull();
  });
});

describe("showsDownloadProgress", () => {
  it("always shows the bar while downloading, even below 100%", () => {
    expect(showsDownloadProgress("downloading", 0.42)).toBe(true);
  });

  it("keeps the bar at 100% through the completed and importing steps", () => {
    expect(showsDownloadProgress("completed", 1)).toBe(true);
    expect(showsDownloadProgress("importing", 1)).toBe(true);
  });

  it("hides the bar once the transfer stopped before reaching 100%", () => {
    expect(showsDownloadProgress("importing", 0.6)).toBe(false);
    expect(showsDownloadProgress("error", 1)).toBe(false);
    expect(showsDownloadProgress("imported", 1)).toBe(false);
  });
});

describe("isTelegramDownloadRetryable", () => {
  it("allows retrying failed and imported downloads", () => {
    expect(isTelegramDownloadRetryable("failed")).toBe(true);
    expect(isTelegramDownloadRetryable("imported")).toBe(true);
  });

  it("does not allow retrying active, available or dismissed downloads", () => {
    for (const status of ["queued", "downloading", "available", "dismissed", "unknown"]) {
      expect(isTelegramDownloadRetryable(status)).toBe(false);
    }
  });
});

describe("fetchSharedFile", () => {
  it("returns a File built from the response", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response("bytes", {
        status: 200,
        headers: {
          "content-type": "application/vnd.comicbook+zip",
          "content-disposition": 'attachment; filename="GON T06.cbz"',
        },
      })
    );
    vi.stubGlobal("fetch", fetchMock);

    const file = await fetchSharedFile("/api/books/b1/file");

    expect(fetchMock).toHaveBeenCalledWith("/api/books/b1/file");
    expect(file.name).toBe("GON T06.cbz");
    expect(file.type).toBe("application/vnd.comicbook+zip");
    expect(await file.text()).toBe("bytes");
  });

  it("falls back to a default name without a disposition header", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(new Response(new Uint8Array([1]), { status: 200 }))
    );

    const file = await fetchSharedFile("/api/books/b1/file");

    expect(file.name).toBe("book");
  });

  it("throws on a failed response", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("nope", { status: 404 })));

    await expect(fetchSharedFile("/api/books/b1/file")).rejects.toThrow("download failed (404)");
  });
});
