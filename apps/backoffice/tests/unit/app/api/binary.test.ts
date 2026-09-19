// @vitest-environment node
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { jsonRequest, routeCtx } from "./helpers";

vi.mock("@/lib/api", async () => {
  const { createApiMock } = await import("./helpers");
  return createApiMock();
});

import { GET as getPage } from "@/app/api/books/[bookId]/pages/[pageNum]/route";
import { GET as getThumbnail } from "@/app/api/books/[bookId]/thumbnail/route";

const fetchMock = vi.fn();

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("GET /api/books/[bookId]/pages/[pageNum]", () => {
  it("proxies the image and forwards cache headers", async () => {
    fetchMock.mockResolvedValue(
      new Response("image-bytes", {
        status: 200,
        headers: { "content-type": "image/jpeg", etag: "etag-1" },
      })
    );
    const res = await getPage(
      jsonRequest("/api/books/b1/pages/3?format=jpeg&width=1200&quality=80", {
        headers: { "if-none-match": "etag-0" },
      }),
      routeCtx({ bookId: "b1", pageNum: "3" })
    );
    expect(fetchMock).toHaveBeenCalledWith(
      "http://api:7080/books/b1/pages/3?format=jpeg&width=1200&quality=80",
      expect.objectContaining({ cache: "no-store" })
    );
    expect(res.status).toBe(200);
    expect(res.headers.get("content-type")).toBe("image/jpeg");
    expect(res.headers.get("etag")).toBe("etag-1");
    expect(res.headers.get("cache-control")).toBe("public, max-age=300, must-revalidate");
    expect(await res.text()).toBe("image-bytes");
  });

  it("uses webp by default and omits empty width/quality", async () => {
    fetchMock.mockResolvedValue(new Response("x", { status: 200 }));
    await getPage(jsonRequest("/api/books/b1/pages/1"), routeCtx({ bookId: "b1", pageNum: "1" }));
    expect(fetchMock).toHaveBeenCalledWith(
      "http://api:7080/books/b1/pages/1?format=webp",
      expect.anything()
    );
  });

  it("forwards a 304 as-is", async () => {
    fetchMock.mockResolvedValue(new Response(null, { status: 304 }));
    const res = await getPage(jsonRequest("/api/books/b1/pages/1"), routeCtx({ bookId: "b1", pageNum: "1" }));
    expect(res.status).toBe(304);
  });

  it("propagates upstream error status", async () => {
    fetchMock.mockResolvedValue(new Response("nope", { status: 404 }));
    const res = await getPage(jsonRequest("/api/books/b1/pages/1"), routeCtx({ bookId: "b1", pageNum: "1" }));
    expect(res.status).toBe(404);
    expect(await res.text()).toBe("Failed to fetch image: 404");
  });

  it("returns 500 when fetch throws", async () => {
    fetchMock.mockRejectedValue(new Error("network"));
    const res = await getPage(jsonRequest("/api/books/b1/pages/1"), routeCtx({ bookId: "b1", pageNum: "1" }));
    expect(res.status).toBe(500);
    expect(await res.text()).toBe("Failed to fetch image");
  });
});

describe("GET /api/books/[bookId]/thumbnail", () => {
  it("proxies the thumbnail", async () => {
    fetchMock.mockResolvedValue(
      new Response("thumb", { status: 200, headers: { "content-type": "image/webp", etag: "t-1" } })
    );
    const res = await getThumbnail(jsonRequest("/api/books/b1/thumbnail"), routeCtx({ bookId: "b1" }));
    expect(fetchMock).toHaveBeenCalledWith(
      "http://api:7080/books/b1/thumbnail",
      expect.objectContaining({ cache: "no-store" })
    );
    expect(res.status).toBe(200);
    expect(res.headers.get("cache-control")).toBe("public, max-age=31536000, must-revalidate");
  });

  it("forwards if-none-match to the upstream", async () => {
    fetchMock.mockResolvedValue(new Response(new Uint8Array([1]), { status: 200 }));
    await getThumbnail(
      jsonRequest("/api/books/b1/thumbnail", { headers: { "if-none-match": "etag-9" } }),
      routeCtx({ bookId: "b1" })
    );
    expect(fetchMock).toHaveBeenCalledWith(
      "http://api:7080/books/b1/thumbnail",
      expect.objectContaining({
        headers: { Authorization: "Bearer test-token", "If-None-Match": "etag-9" },
      })
    );
  });

  it("defaults missing content-type to webp", async () => {
    fetchMock.mockResolvedValue(new Response(new Uint8Array([1, 2, 3]), { status: 200 }));
    const res = await getThumbnail(jsonRequest("/api/books/b1/thumbnail"), routeCtx({ bookId: "b1" }));
    expect(res.headers.get("content-type")).toBe("image/webp");
    expect(res.headers.get("etag")).toBeNull();
  });

  it("forwards a 304 as-is", async () => {
    fetchMock.mockResolvedValue(new Response(null, { status: 304 }));
    const res = await getThumbnail(jsonRequest("/api/books/b1/thumbnail"), routeCtx({ bookId: "b1" }));
    expect(res.status).toBe(304);
  });

  it("propagates upstream error status", async () => {
    fetchMock.mockResolvedValue(new Response("no", { status: 500 }));
    const res = await getThumbnail(jsonRequest("/api/books/b1/thumbnail"), routeCtx({ bookId: "b1" }));
    expect(res.status).toBe(500);
  });

  it("returns 500 when fetch throws", async () => {
    fetchMock.mockRejectedValue(new Error("network"));
    const res = await getThumbnail(jsonRequest("/api/books/b1/thumbnail"), routeCtx({ bookId: "b1" }));
    expect(res.status).toBe(500);
    expect(await res.text()).toBe("Failed to fetch thumbnail");
  });
});
