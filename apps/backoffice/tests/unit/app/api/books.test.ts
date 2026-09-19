// @vitest-environment node
import { describe, expect, it, vi } from "vitest";

import { jsonRequest, routeCtx } from "./helpers";

vi.mock("@/lib/api", async () => {
  const { createApiMock } = await import("./helpers");
  return createApiMock();
});
const revalidatePath = vi.hoisted(() => vi.fn());
vi.mock("next/cache", () => ({ revalidatePath, revalidateTag: vi.fn() }));

import { apiFetch, convertBook, updateBook, updateReadingProgress } from "@/lib/api";
import {
  PATCH as patchBook,
  DELETE as deleteBook,
} from "@/app/api/books/[bookId]/route";
import { POST as convert } from "@/app/api/books/[bookId]/convert/route";
import { PATCH as patchProgress } from "@/app/api/books/[bookId]/progress/route";

const mockApi = vi.mocked(apiFetch);

describe("/api/books/[bookId]", () => {
  it("PATCH delegates to updateBook", async () => {
    vi.mocked(updateBook).mockResolvedValue({ id: "b1" } as never);
    const res = await patchBook(
      jsonRequest("/api/books/b1", { method: "PATCH", body: { title: "T" } }),
      routeCtx({ bookId: "b1" })
    );
    expect(updateBook).toHaveBeenCalledWith("b1", { title: "T" });
    expect(res.status).toBe(200);
  });

  it("PATCH returns 500 on error", async () => {
    vi.mocked(updateBook).mockRejectedValue(new Error("bad"));
    expect((await patchBook(jsonRequest("/api/books/b1", { method: "PATCH", body: {} }), routeCtx({ bookId: "b1" }))).status).toBe(500);
  });

  it("DELETE proxies the API", async () => {
    mockApi.mockResolvedValue({ deleted: true });
    const res = await deleteBook(jsonRequest("/api/books/b1", { method: "DELETE" }), routeCtx({ bookId: "b1" }));
    expect(mockApi).toHaveBeenCalledWith("/books/b1", { method: "DELETE" });
    expect(res.status).toBe(200);
  });

  it("DELETE returns 500 on error", async () => {
    mockApi.mockRejectedValue(new Error("bad"));
    expect((await deleteBook(jsonRequest("/api/books/b1", { method: "DELETE" }), routeCtx({ bookId: "b1" }))).status).toBe(500);
  });
});

describe("/api/books/[bookId]/convert", () => {
  it("POST delegates to convertBook", async () => {
    vi.mocked(convertBook).mockResolvedValue({ job_id: "j1" } as never);
    const res = await convert(jsonRequest("/api/books/b1/convert", { method: "POST" }), routeCtx({ bookId: "b1" }));
    expect(convertBook).toHaveBeenCalledWith("b1");
    expect(res.status).toBe(200);
  });

  it("maps a 409 conflict", async () => {
    vi.mocked(convertBook).mockRejectedValue(new Error("request failed (409)"));
    const res = await convert(jsonRequest("/api/books/b1/convert", { method: "POST" }), routeCtx({ bookId: "b1" }));
    expect(res.status).toBe(409);
  });

  it("returns 500 for other errors", async () => {
    vi.mocked(convertBook).mockRejectedValue(new Error("boom"));
    expect((await convert(jsonRequest("/api/books/b1/convert", { method: "POST" }), routeCtx({ bookId: "b1" }))).status).toBe(500);
  });
});

describe("/api/books/[bookId]/progress", () => {
  it("updates progress and revalidates the series path", async () => {
    vi.mocked(updateReadingProgress).mockResolvedValue({ ok: true } as never);
    mockApi.mockResolvedValue({ series_id: "s1" });
    const res = await patchProgress(
      jsonRequest("/api/books/b1/progress", { method: "PATCH", body: { status: "read", current_page: 3 } }),
      routeCtx({ bookId: "b1" })
    );
    expect(updateReadingProgress).toHaveBeenCalledWith("b1", "read", 3);
    expect(revalidatePath).toHaveBeenCalledWith("/series");
    expect(revalidatePath).toHaveBeenCalledWith("/books/b1");
    expect(revalidatePath).toHaveBeenCalledWith("/series/s1");
    expect(res.status).toBe(200);
  });

  it("tolerates a failing book lookup", async () => {
    vi.mocked(updateReadingProgress).mockResolvedValue({ ok: true } as never);
    mockApi.mockRejectedValue(new Error("nope"));
    const res = await patchProgress(
      jsonRequest("/api/books/b1/progress", { method: "PATCH", body: { status: "reading" } }),
      routeCtx({ bookId: "b1" })
    );
    expect(res.status).toBe(200);
    expect(revalidatePath).not.toHaveBeenCalledWith("/series/s1");
  });

  it("returns 500 when the update fails", async () => {
    vi.mocked(updateReadingProgress).mockRejectedValue(new Error("bad"));
    mockApi.mockResolvedValue({ series_id: "s1" });
    expect(
      (await patchProgress(jsonRequest("/api/books/b1/progress", { method: "PATCH", body: { status: "read" } }), routeCtx({ bookId: "b1" }))).status
    ).toBe(500);
  });
});
