// @vitest-environment node
import { describe, expect, it, vi } from "vitest";

import { jsonRequest, routeCtx } from "./helpers";

vi.mock("@/lib/api", async () => {
  const { createApiMock } = await import("./helpers");
  return createApiMock();
});
vi.mock("next/cache", () => ({ revalidatePath: vi.fn(), revalidateTag: vi.fn() }));

import { apiFetch, markSeriesRead } from "@/lib/api";
import { GET as listSeries } from "@/app/api/series/route";
import { GET as searchSeries } from "@/app/api/series/search/route";
import { GET as listGenres } from "@/app/api/series/genres/route";
import { GET as listStatuses } from "@/app/api/series/statuses/route";
import { GET as listProviderStatuses } from "@/app/api/series/provider-statuses/route";
import { POST as createSeries } from "@/app/api/series/create/route";
import { POST as markRead } from "@/app/api/series/mark-read/route";
import { GET as listArchived } from "@/app/api/series/archived/route";
import { GET as getArchived } from "@/app/api/series/archived/[id]/route";
import { PATCH as patchSeries, DELETE as deleteSeries } from "@/app/api/series/[seriesId]/route";
import { GET as getSeriesMetadata } from "@/app/api/series/[seriesId]/metadata/route";
import { POST as mergeSeries } from "@/app/api/series/[seriesId]/merge/route";
import { PUT as putRating, DELETE as deleteRating } from "@/app/api/series/[seriesId]/rating/route";
import { POST as renameBooks } from "@/app/api/series/[seriesId]/rename-books/route";
import {
  GET as getAnilist,
  POST as linkAnilist,
  DELETE as unlinkAnilist,
} from "@/app/api/series/[seriesId]/anilist/route";

const mockApi = vi.mocked(apiFetch);

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

  it("PATCH series forwards body", async () => {
    mockApi.mockResolvedValue({ id: "s1" });
    const res = await patchSeries(
      jsonRequest("/api/series/s1", { method: "PATCH", body: { name: "New" } }),
      routeCtx({ seriesId: "s1" })
    );
    expect(mockApi).toHaveBeenCalledWith("/series/s1", expect.objectContaining({ method: "PATCH" }));
    expect(res.status).toBe(200);
  });

  it("DELETE series forwards", async () => {
    mockApi.mockResolvedValue({ deleted: true });
    const res = await deleteSeries(jsonRequest("/api/series/s1", { method: "DELETE" }), routeCtx({ seriesId: "s1" }));
    expect(mockApi).toHaveBeenCalledWith("/series/s1", { method: "DELETE" });
    expect(res.status).toBe(200);
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
      jsonRequest("/api/series/s1/merge", { method: "POST", body: { target: "s2" } }),
      routeCtx({ seriesId: "s1" })
    );
    expect(res.status).toBe(200);
  });

  it("PUT rating returns 204", async () => {
    mockApi.mockResolvedValue(undefined);
    const res = await putRating(jsonRequest("/api/series/s1/rating", { method: "PUT", body: { rating: 4 } }), routeCtx({ seriesId: "s1" }));
    expect(mockApi).toHaveBeenCalledWith("/series/s1/rating", expect.objectContaining({ method: "PUT" }));
    expect(res.status).toBe(204);
  });

  it("DELETE rating returns 204", async () => {
    mockApi.mockResolvedValue(undefined);
    const res = await deleteRating(jsonRequest("/api/series/s1/rating", { method: "DELETE" }), routeCtx({ seriesId: "s1" }));
    expect(res.status).toBe(204);
  });

  it("PUT rating returns 500 on error", async () => {
    mockApi.mockRejectedValue(new Error("bad"));
    expect((await putRating(jsonRequest("/api/series/s1/rating", { method: "PUT", body: {} }), routeCtx({ seriesId: "s1" }))).status).toBe(500);
  });

  it("POST rename-books forwards", async () => {
    mockApi.mockResolvedValue({ renamed: 2 });
    const res = await renameBooks(
      jsonRequest("/api/series/s1/rename-books", { method: "POST", body: { pattern: "{n}" } }),
      routeCtx({ seriesId: "s1" })
    );
    expect(res.status).toBe(200);
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
