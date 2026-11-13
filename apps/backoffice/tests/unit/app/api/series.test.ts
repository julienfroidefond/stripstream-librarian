// @vitest-environment node
import { NextRequest } from "next/server";
import { revalidatePath, revalidateTag } from "next/cache";
import { describe, expect, it, vi } from "vitest";

import { jsonRequest, routeCtx } from "./helpers";

vi.mock("@/lib/api", async () => {
  const { createApiMock } = await import("./helpers");
  return createApiMock();
});
vi.mock("next/cache", () => ({ revalidatePath: vi.fn(), revalidateTag: vi.fn() }));

import { apiFetch, markSeriesRead } from "@/lib/api";
import { ApiError } from "@/lib/api/errors";
import { GET as listSeries } from "@/app/api/series/route";
import { GET as searchSeries } from "@/app/api/series/search/route";
import { GET as listGenres } from "@/app/api/series/genres/route";
import { GET as listStatuses } from "@/app/api/series/statuses/route";
import { GET as listProviderStatuses } from "@/app/api/series/provider-statuses/route";
import { POST as createSeries } from "@/app/api/series/create/route";
import { POST as markRead } from "@/app/api/series/mark-read/route";
import { GET as listArchived } from "@/app/api/series/archived/route";
import { GET as getArchived } from "@/app/api/series/archived/[id]/route";
import { GET as listOrphans, DELETE as deleteOrphans } from "@/app/api/series/orphans/route";
import { PATCH as patchSeries, DELETE as deleteSeries } from "@/app/api/series/[seriesId]/route";
import { GET as getSeriesMetadata } from "@/app/api/series/[seriesId]/metadata/route";
import { POST as mergeSeries } from "@/app/api/series/[seriesId]/merge/route";
import { PUT as putRating, DELETE as deleteRating } from "@/app/api/series/[seriesId]/rating/route";
import { PUT as putShelf, DELETE as deleteShelf } from "@/app/api/series/[seriesId]/shelf/route";
import { POST as renameBooks } from "@/app/api/series/[seriesId]/rename-books/route";
import {
  GET as getAnilist,
  POST as linkAnilist,
  DELETE as unlinkAnilist,
} from "@/app/api/series/[seriesId]/anilist/route";

const mockApi = vi.mocked(apiFetch);
const mockRevalidatePath = vi.mocked(revalidatePath);
const mockRevalidateTag = vi.mocked(revalidateTag);

const ID_A = "11111111-1111-4111-8111-111111111111";
const ID_B = "22222222-2222-4222-8222-222222222222";
const ID_C = "33333333-3333-4333-8333-333333333333";

function uuid(n: number): string {
  return `00000000-0000-4000-8000-${String(n).padStart(12, "0")}`;
}

describe("/api/series", () => {
  it("GET forwards the query string", async () => {
    mockApi.mockResolvedValue({ items: [], total: 0 });
    const res = await listSeries(jsonRequest("/api/series?page=2"));
    expect(mockApi).toHaveBeenCalledWith("/series?page=2");
    expect(await res.json()).toEqual({ items: [], total: 0 });
  });

  it("GET returns 500 on error", async () => {
    mockApi.mockRejectedValue(new Error("boom"));
    const res = await listSeries(jsonRequest("/api/series"));
    expect(res.status).toBe(500);
  });

  it("search forwards the query string", async () => {
    mockApi.mockResolvedValue({ items: [], total: 0, page: 1, limit: 25 });
    const res = await searchSeries(jsonRequest("/api/series/search?q=dune"));
    expect(mockApi).toHaveBeenCalledWith("/series?q=dune");
    expect(res.status).toBe(200);
  });

  it("search returns 500 on error", async () => {
    mockApi.mockRejectedValue(new Error("nope"));
    expect((await searchSeries(jsonRequest("/api/series/search"))).status).toBe(500);
  });

  it("GET genres passthrough", async () => {
    mockApi.mockResolvedValue(["Action"]);
    expect(await (await listGenres()).json()).toEqual(["Action"]);
  });

  it("GET statuses passthrough", async () => {
    mockApi.mockResolvedValue(["ongoing"]);
    expect(await (await listStatuses()).json()).toEqual(["ongoing"]);
  });

  it("GET provider statuses passthrough", async () => {
    mockApi.mockResolvedValue(["reading"]);
    expect(await (await listProviderStatuses()).json()).toEqual(["reading"]);
  });

  it("POST create forwards the body", async () => {
    mockApi.mockResolvedValue({ id: "1" });
    const res = await createSeries(jsonRequest("/api/series/create", { method: "POST", body: { name: "X" } }));
    expect(mockApi).toHaveBeenCalledWith("/series/create", expect.objectContaining({ method: "POST" }));
    expect(res.status).toBe(200);
    expect(mockRevalidateTag).toHaveBeenCalledWith("series", { expire: 0 });
    expect(mockRevalidatePath).toHaveBeenCalledWith("/series");
  });

  it("POST mark-read calls markSeriesRead with defaults", async () => {
    vi.mocked(markSeriesRead).mockResolvedValue({ updated: 1 });
    const res = await markRead(
      jsonRequest("/api/series/mark-read", { method: "POST", body: { series: ["a"] } })
    );
    expect(markSeriesRead).toHaveBeenCalledWith(["a"], "read");
    expect(res.status).toBe(200);
  });

  it("POST mark-read honours an explicit status", async () => {
    vi.mocked(markSeriesRead).mockResolvedValue({ updated: 1 });
    await markRead(
      jsonRequest("/api/series/mark-read", { method: "POST", body: { series: ["a"], status: "unread" } })
    );
    expect(markSeriesRead).toHaveBeenCalledWith(["a"], "unread");
  });

  it("GET archived passthrough", async () => {
    mockApi.mockResolvedValue([{ id: "1" }]);
    expect(await (await listArchived()).json()).toEqual([{ id: "1" }]);
  });

  it("GET archived by id passthrough", async () => {
    mockApi.mockResolvedValue({ id: "1" });
    const res = await getArchived(jsonRequest("/api/series/archived/1"), routeCtx({ id: "1" }));
    expect(mockApi).toHaveBeenCalledWith("/admin/series/archived/1");
    expect(res.status).toBe(200);
  });

  it("GET orphans forwards pagination", async () => {
    mockApi.mockResolvedValue({ items: [], total: 0, page: 2, limit: 10 });
    const res = await listOrphans(jsonRequest("/api/series/orphans?page=2&limit=10"));
    expect(mockApi).toHaveBeenCalledWith("/admin/series/orphans?page=2&limit=10");
    expect(await res.json()).toEqual({ items: [], total: 0, page: 2, limit: 10 });
  });

  it("GET orphans returns 500 on error", async () => {
    mockApi.mockRejectedValue(new Error("boom"));
    const res = await listOrphans(jsonRequest("/api/series/orphans"));
    expect(res.status).toBe(500);
    expect(await res.json()).toEqual({ error: "boom" });
  });

  it.each([
    ["no body", undefined],
    ["empty object", {}],
    ["null ids", { ids: null }],
    ["empty ids", { ids: [] }],
    ["all false", { all: false }],
    ["non-string ids", { ids: [ID_A, 3] }],
    ["non-uuid ids", { ids: ["a", "b"] }],
    ["a path traversal id", { ids: [`x/../../../../books/${ID_A}`] }],
    ["a dot-segment id", { ids: [`${ID_A}/../../../../books/${ID_B}`] }],
    ["all together with ids", { all: true, ids: [ID_A] }],
  ])("DELETE orphans rejects %s instead of deleting everything", async (_label, body) => {
    const res = await deleteOrphans(
      jsonRequest("/api/series/orphans", { method: "DELETE", body })
    );

    expect(res.status).toBe(400);
    expect(mockApi).not.toHaveBeenCalled();
  });

  it("DELETE orphans rejects malformed JSON", async () => {
    const request = new NextRequest("http://localhost:7082/api/series/orphans", {
      method: "DELETE",
      body: "{",
      headers: { "content-type": "application/json" },
    });

    const res = await deleteOrphans(request);

    expect(res.status).toBe(400);
    expect(mockApi).not.toHaveBeenCalled();
  });

  it("DELETE orphans calls the dedicated endpoint for each explicit id", async () => {
    mockApi.mockResolvedValue({ deleted: true });
    const res = await deleteOrphans(
      jsonRequest("/api/series/orphans", {
        method: "DELETE",
        body: { ids: [ID_A, ID_B] },
      })
    );

    expect(mockApi).toHaveBeenCalledTimes(2);
    expect(mockApi).toHaveBeenNthCalledWith(1, `/admin/series/orphans/${ID_A}`, {
      method: "DELETE",
    });
    expect(mockApi).toHaveBeenNthCalledWith(2, `/admin/series/orphans/${ID_B}`, {
      method: "DELETE",
    });
    expect(await res.json()).toEqual({ deleted: 2, skipped: 0, failed: [] });
  });

  it("DELETE orphans deduplicates repeated ids", async () => {
    mockApi.mockResolvedValue({ deleted: true });
    const res = await deleteOrphans(
      jsonRequest("/api/series/orphans", {
        method: "DELETE",
        body: { ids: [ID_A, ID_A, ID_B, ID_B] },
      })
    );

    expect(mockApi).toHaveBeenCalledTimes(2);
    expect(mockApi).toHaveBeenNthCalledWith(1, `/admin/series/orphans/${ID_A}`, {
      method: "DELETE",
    });
    expect(mockApi).toHaveBeenNthCalledWith(2, `/admin/series/orphans/${ID_B}`, {
      method: "DELETE",
    });
    expect(await res.json()).toEqual({ deleted: 2, skipped: 0, failed: [] });
  });

  it("DELETE orphans treats a typed 404 as already deleted", async () => {
    mockApi.mockImplementation((path: string) => {
      if (path === `/admin/series/orphans/${ID_B}`) {
        return Promise.reject(new ApiError(404, "not found"));
      }
      return Promise.resolve({ deleted: true });
    });

    const res = await deleteOrphans(
      jsonRequest("/api/series/orphans", {
        method: "DELETE",
        body: { ids: [ID_A, ID_B] },
      })
    );

    expect(await res.json()).toEqual({ deleted: 1, skipped: 1, failed: [] });
  });

  it("DELETE orphans does not treat a 500 message containing (404) as not found", async () => {
    mockApi.mockRejectedValue(new Error("API failed (500): nested (404) mention"));

    const res = await deleteOrphans(
      jsonRequest("/api/series/orphans", { method: "DELETE", body: { ids: [ID_A] } })
    );

    expect(await res.json()).toEqual({ deleted: 0, skipped: 0, failed: [ID_A] });
  });

  it("DELETE orphans keeps going after a partial failure", async () => {
    mockApi.mockImplementation((path: string) => {
      if (path === `/admin/series/orphans/${ID_B}`) {
        return Promise.reject(new ApiError(409, "no longer an orphan"));
      }
      return Promise.resolve({ deleted: true });
    });

    const res = await deleteOrphans(
      jsonRequest("/api/series/orphans", {
        method: "DELETE",
        body: { ids: [ID_A, ID_B, ID_C] },
      })
    );

    expect(mockApi).toHaveBeenCalledTimes(3);
    expect(await res.json()).toEqual({ deleted: 2, skipped: 0, failed: [ID_B] });
  });

  it("DELETE orphans purges every orphan only when all is explicitly true", async () => {
    mockApi
      .mockResolvedValueOnce({
        items: [{ id: ID_A }, { id: ID_B }],
        total: 2,
        page: 1,
        limit: 200,
      })
      .mockResolvedValue({ deleted: true });

    const res = await deleteOrphans(
      jsonRequest("/api/series/orphans", { method: "DELETE", body: { all: true } })
    );

    expect(mockApi).toHaveBeenNthCalledWith(1, "/admin/series/orphans?page=1&limit=200");
    expect(mockApi).toHaveBeenNthCalledWith(2, `/admin/series/orphans/${ID_A}`, {
      method: "DELETE",
    });
    expect(mockApi).toHaveBeenNthCalledWith(3, `/admin/series/orphans/${ID_B}`, {
      method: "DELETE",
    });
    expect(await res.json()).toEqual({ deleted: 2, skipped: 0, failed: [] });
  });

  it("DELETE orphans excludes a series with an in-progress download from an all purge", async () => {
    mockApi
      .mockResolvedValueOnce({
        items: [
          { id: ID_A, has_active_download: false },
          { id: ID_B, has_active_download: true },
        ],
        total: 2,
        page: 1,
        limit: 200,
      })
      .mockResolvedValue({ deleted: true });

    const res = await deleteOrphans(
      jsonRequest("/api/series/orphans", { method: "DELETE", body: { all: true } })
    );

    expect(mockApi).toHaveBeenCalledTimes(2);
    expect(mockApi).toHaveBeenNthCalledWith(2, `/admin/series/orphans/${ID_A}`, {
      method: "DELETE",
    });
    expect(await res.json()).toEqual({ deleted: 1, skipped: 0, failed: [] });
  });

  it("DELETE orphans walks multiple pages for an all purge", async () => {
    mockApi
      .mockResolvedValueOnce({
        items: Array.from({ length: 200 }, (_, i) => ({ id: uuid(i + 1) })),
        total: 201,
        page: 1,
        limit: 200,
      })
      .mockResolvedValueOnce({ items: [{ id: uuid(201) }], total: 201, page: 2, limit: 200 })
      .mockResolvedValue({ deleted: true });

    const res = await deleteOrphans(
      jsonRequest("/api/series/orphans", { method: "DELETE", body: { all: true } })
    );

    expect(mockApi).toHaveBeenNthCalledWith(2, "/admin/series/orphans?page=2&limit=200");
    expect(mockApi).toHaveBeenCalledTimes(203);
    expect(await res.json()).toEqual({ deleted: 201, skipped: 0, failed: [] });
  });

  it("DELETE orphans keeps walking when the API clamps the page limit", async () => {
    mockApi
      .mockResolvedValueOnce({
        items: Array.from({ length: 50 }, (_, i) => ({ id: uuid(i + 1) })),
        total: 51,
        page: 1,
        limit: 50,
      })
      .mockResolvedValueOnce({ items: [{ id: uuid(51) }], total: 51, page: 2, limit: 50 })
      .mockResolvedValue({ deleted: true });

    const res = await deleteOrphans(
      jsonRequest("/api/series/orphans", { method: "DELETE", body: { all: true } })
    );

    expect(mockApi).toHaveBeenNthCalledWith(2, "/admin/series/orphans?page=2&limit=200");
    expect(await res.json()).toEqual({ deleted: 51, skipped: 0, failed: [] });
  });

  it("DELETE orphans returns 500 when listing all orphans fails", async () => {
    mockApi.mockRejectedValue(new Error("nope"));
    const res = await deleteOrphans(
      jsonRequest("/api/series/orphans", { method: "DELETE", body: { all: true } })
    );
    expect(res.status).toBe(500);
  });

  it("PATCH series forwards body and revalidates series caches", async () => {
    mockApi.mockResolvedValue({ id: "s1" });
    const res = await patchSeries(
      jsonRequest("/api/series/s1", { method: "PATCH", body: { name: "New" } }),
      routeCtx({ seriesId: "s1" })
    );
    expect(mockApi).toHaveBeenCalledWith("/series/s1", expect.objectContaining({ method: "PATCH" }));
    expect(mockRevalidatePath).toHaveBeenCalledWith("/series");
    expect(mockRevalidatePath).toHaveBeenCalledWith("/series/s1");
    expect(mockRevalidateTag).toHaveBeenCalledWith("series", { expire: 0 });
    expect(mockRevalidateTag).toHaveBeenCalledWith("series:s1", { expire: 0 });
    expect(res.status).toBe(200);
  });

  it("DELETE series forwards", async () => {
    mockApi.mockResolvedValue({ deleted: true });
    const res = await deleteSeries(jsonRequest("/api/series/s1", { method: "DELETE" }), routeCtx({ seriesId: "s1" }));
    expect(mockApi).toHaveBeenCalledWith("/series/s1", { method: "DELETE" });
    expect(res.status).toBe(200);
    // DELETE now expires the same caches as PATCH (and more), not only the list path.
    expect(mockRevalidateTag).toHaveBeenCalledWith("series", { expire: 0 });
    expect(mockRevalidateTag).toHaveBeenCalledWith("series:s1", { expire: 0 });
    expect(mockRevalidatePath).toHaveBeenCalledWith("/series/s1");
  });

  it("PATCH series returns 500 on error", async () => {
    mockApi.mockRejectedValue(new Error("bad"));
    expect((await patchSeries(jsonRequest("/api/series/s1", { method: "PATCH", body: {} }), routeCtx({ seriesId: "s1" }))).status).toBe(500);
  });

  it("DELETE series returns 500 on error", async () => {
    mockApi.mockRejectedValue(new Error("bad"));
    expect((await deleteSeries(jsonRequest("/api/series/s1", { method: "DELETE" }), routeCtx({ seriesId: "s1" }))).status).toBe(500);
  });

  it("GET series metadata returns 404 on error", async () => {
    mockApi.mockRejectedValue(new Error("not found"));
    const res = await getSeriesMetadata(jsonRequest("/api/series/s1/metadata"), routeCtx({ seriesId: "s1" }));
    expect(res.status).toBe(500);
  });

  it("GET series metadata success", async () => {
    mockApi.mockResolvedValue({ links: [] });
    expect((await getSeriesMetadata(jsonRequest("/api/series/s1/metadata"), routeCtx({ seriesId: "s1" }))).status).toBe(200);
  });

  it("POST merge forwards", async () => {
    mockApi.mockResolvedValue({ merged: true });
    const res = await mergeSeries(
      jsonRequest("/api/series/s1/merge", { method: "POST", body: { source_id: "s2" } }),
      routeCtx({ seriesId: "s1" })
    );
    expect(res.status).toBe(200);
    expect(mockRevalidateTag).toHaveBeenCalledWith("series:s1", { expire: 0 });
    expect(mockRevalidateTag).toHaveBeenCalledWith("series:s2", { expire: 0 });
    expect(mockRevalidateTag).toHaveBeenCalledWith("books", { expire: 0 });
  });

  it("PUT rating returns 204", async () => {
    mockApi.mockResolvedValue(undefined);
    const res = await putRating(jsonRequest("/api/series/s1/rating", { method: "PUT", body: { rating: 4 } }), routeCtx({ seriesId: "s1" }));
    expect(mockApi).toHaveBeenCalledWith("/series/s1/rating", expect.objectContaining({ method: "PUT" }));
    expect(mockRevalidateTag).toHaveBeenCalledWith("series", { expire: 0 });
    expect(mockRevalidateTag).toHaveBeenCalledWith("series:s1", { expire: 0 });
    expect(res.status).toBe(204);
  });

  it("DELETE rating returns 204", async () => {
    mockApi.mockResolvedValue(undefined);
    const res = await deleteRating(jsonRequest("/api/series/s1/rating", { method: "DELETE" }), routeCtx({ seriesId: "s1" }));
    expect(mockRevalidateTag).toHaveBeenCalledWith("series:s1", { expire: 0 });
    expect(res.status).toBe(204);
  });

  it("PUT rating returns 500 on error", async () => {
    mockApi.mockRejectedValue(new Error("bad"));
    expect((await putRating(jsonRequest("/api/series/s1/rating", { method: "PUT", body: {} }), routeCtx({ seriesId: "s1" }))).status).toBe(500);
  });

  it("PUT shelf returns 204 and revalidates the series", async () => {
    mockApi.mockResolvedValue(undefined);
    const res = await putShelf(
      jsonRequest("/api/series/s1/shelf", { method: "PUT" }),
      routeCtx({ seriesId: "s1" })
    );
    expect(mockApi).toHaveBeenCalledWith("/series/s1/shelf", expect.objectContaining({ method: "PUT" }));
    expect(mockRevalidatePath).toHaveBeenCalledWith("/series/s1");
    expect(mockRevalidatePath).toHaveBeenCalledWith("/series");
    expect(mockRevalidateTag).toHaveBeenCalledWith("series", { expire: 0 });
    expect(mockRevalidateTag).toHaveBeenCalledWith("series:s1", { expire: 0 });
    expect(res.status).toBe(204);
  });

  it("DELETE shelf returns 204 and revalidates the series", async () => {
    mockApi.mockResolvedValue(undefined);
    const res = await deleteShelf(
      jsonRequest("/api/series/s1/shelf", { method: "DELETE" }),
      routeCtx({ seriesId: "s1" })
    );
    expect(mockApi).toHaveBeenCalledWith("/series/s1/shelf", expect.objectContaining({ method: "DELETE" }));
    expect(mockRevalidateTag).toHaveBeenCalledWith("series:s1", { expire: 0 });
    expect(res.status).toBe(204);
  });

  it("PUT shelf returns 500 on error", async () => {
    mockApi.mockRejectedValue(new Error("bad"));
    expect((await putShelf(jsonRequest("/api/series/s1/shelf", { method: "PUT" }), routeCtx({ seriesId: "s1" }))).status).toBe(500);
  });

  it("POST rename-books forwards", async () => {
    mockApi.mockResolvedValue({ renamed: 2 });
    const res = await renameBooks(
      jsonRequest("/api/series/s1/rename-books", { method: "POST", body: { pattern: "{n}" } }),
      routeCtx({ seriesId: "s1" })
    );
    expect(res.status).toBe(200);
    expect(mockRevalidateTag).toHaveBeenCalledWith("books", { expire: 0 });
    expect(mockRevalidateTag).toHaveBeenCalledWith("series:s1", { expire: 0 });
  });

  it("GET anilist passthrough", async () => {
    mockApi.mockResolvedValue({ linked: true });
    expect(await (await getAnilist(jsonRequest("/api/series/s1/anilist"), routeCtx({ seriesId: "s1" }))).json()).toEqual({ linked: true });
  });

  it("GET anilist returns 404 on error", async () => {
    mockApi.mockRejectedValue(new Error("nope"));
    expect((await getAnilist(jsonRequest("/api/series/s1/anilist"), routeCtx({ seriesId: "s1" }))).status).toBe(404);
  });

  it("POST anilist link forwards", async () => {
    mockApi.mockResolvedValue({ linked: true });
    const res = await linkAnilist(
      jsonRequest("/api/series/s1/anilist", { method: "POST", body: { id: 42 } }),
      routeCtx({ seriesId: "s1" })
    );
    expect(mockApi).toHaveBeenCalledWith("/series/s1/anilist/link", expect.objectContaining({ method: "POST" }));
    expect(res.status).toBe(200);
  });

  it("DELETE anilist unlink forwards", async () => {
    mockApi.mockResolvedValue({ linked: false });
    const res = await unlinkAnilist(jsonRequest("/api/series/s1/anilist", { method: "DELETE" }), routeCtx({ seriesId: "s1" }));
    expect(mockApi).toHaveBeenCalledWith("/series/s1/anilist/unlink", { method: "DELETE" });
    expect(res.status).toBe(200);
  });

  it("POST anilist returns 500 with the error message", async () => {
    mockApi.mockRejectedValue(new Error("link failed"));
    const res = await linkAnilist(jsonRequest("/api/series/s1/anilist", { method: "POST", body: {} }), routeCtx({ seriesId: "s1" }));
    expect(res.status).toBe(500);
    expect(await res.json()).toEqual({ error: "link failed" });
  });

  it("POST anilist falls back on a non-error failure", async () => {
    mockApi.mockRejectedValue("boom");
    const res = await linkAnilist(jsonRequest("/api/series/s1/anilist", { method: "POST", body: {} }), routeCtx({ seriesId: "s1" }));
    expect(await res.json()).toEqual({ error: "Failed to link series" });
  });

  it("DELETE anilist returns 500 with the error message", async () => {
    mockApi.mockRejectedValue(new Error("unlink failed"));
    const res = await unlinkAnilist(jsonRequest("/api/series/s1/anilist", { method: "DELETE" }), routeCtx({ seriesId: "s1" }));
    expect(res.status).toBe(500);
    expect(await res.json()).toEqual({ error: "unlink failed" });
  });

  it("DELETE anilist falls back on a non-error failure", async () => {
    mockApi.mockRejectedValue("boom");
    const res = await unlinkAnilist(jsonRequest("/api/series/s1/anilist", { method: "DELETE" }), routeCtx({ seriesId: "s1" }));
    expect(await res.json()).toEqual({ error: "Failed to unlink series" });
  });
});
