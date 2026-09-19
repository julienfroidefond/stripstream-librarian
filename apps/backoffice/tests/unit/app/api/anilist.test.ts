// @vitest-environment node
import { describe, expect, it, vi } from "vitest";

import { jsonRequest, routeCtx } from "./helpers";

vi.mock("@/lib/api", async () => {
  const { createApiMock } = await import("./helpers");
  return createApiMock();
});

import { apiFetch } from "@/lib/api";
import { PATCH as patchLibrary } from "@/app/api/anilist/libraries/[id]/route";
import { GET as links } from "@/app/api/anilist/links/route";
import { POST as pull } from "@/app/api/anilist/pull/route";
import { POST as search } from "@/app/api/anilist/search/route";
import {
  GET as getSeries,
  POST as linkSeries,
  DELETE as unlinkSeries,
} from "@/app/api/anilist/series/[libraryId]/[seriesId]/route";
import { GET as status } from "@/app/api/anilist/status/route";
import { GET as syncPreview } from "@/app/api/anilist/sync/preview/route";
import { POST as sync } from "@/app/api/anilist/sync/route";
import { GET as unlinked } from "@/app/api/anilist/unlinked/route";

const mockApi = vi.mocked(apiFetch);

describe("/api/anilist", () => {
  it("PATCH library forwards body", async () => {
    mockApi.mockResolvedValue({ ok: true });
    const res = await patchLibrary(jsonRequest("/api/anilist/libraries/l1", { method: "PATCH", body: { enabled: true } }), routeCtx({ id: "l1" }));
    expect(mockApi).toHaveBeenCalledWith("/anilist/libraries/l1", expect.objectContaining({ method: "PATCH" }));
    expect(res.status).toBe(200);
  });

  it("PATCH library returns 500 on error", async () => {
    mockApi.mockRejectedValue(new Error("bad"));
    expect((await patchLibrary(jsonRequest("/api/anilist/libraries/l1", { method: "PATCH", body: {} }), routeCtx({ id: "l1" }))).status).toBe(500);
  });

  it("GET links proxies", async () => {
    mockApi.mockResolvedValue([{ id: "1" }]);
    expect(await (await links()).json()).toEqual([{ id: "1" }]);
  });

  it("POST pull posts an empty object", async () => {
    mockApi.mockResolvedValue({ updated: 0 });
    await pull();
    expect(mockApi).toHaveBeenCalledWith("/anilist/pull", { method: "POST", body: "{}" });
  });

  it("POST search forwards body", async () => {
    mockApi.mockResolvedValue([]);
    expect((await search(jsonRequest("/api/anilist/search", { method: "POST", body: { q: "x" } }))).status).toBe(200);
  });

  it("GET series proxies", async () => {
    mockApi.mockResolvedValue({ linked: false });
    await getSeries(jsonRequest("/api/anilist/series/l1/s1"), routeCtx({ libraryId: "l1", seriesId: "s1" }));
    expect(mockApi).toHaveBeenCalledWith("/anilist/series/l1/s1");
  });

  it("GET series returns 404 on error", async () => {
    mockApi.mockRejectedValue(new Error("nope"));
    expect((await getSeries(jsonRequest("/api/anilist/series/l1/s1"), routeCtx({ libraryId: "l1", seriesId: "s1" }))).status).toBe(404);
  });

  it("POST link proxies", async () => {
    mockApi.mockResolvedValue({ linked: true });
    await linkSeries(jsonRequest("/api/anilist/series/l1/s1", { method: "POST", body: { id: 1 } }), routeCtx({ libraryId: "l1", seriesId: "s1" }));
    expect(mockApi).toHaveBeenCalledWith("/anilist/series/l1/s1/link", expect.objectContaining({ method: "POST" }));
  });

  it("DELETE unlink proxies", async () => {
    mockApi.mockResolvedValue({ linked: false });
    await unlinkSeries(jsonRequest("/api/anilist/series/l1/s1", { method: "DELETE" }), routeCtx({ libraryId: "l1", seriesId: "s1" }));
    expect(mockApi).toHaveBeenCalledWith("/anilist/series/l1/s1/unlink", { method: "DELETE" });
  });

  it("POST link returns 500 on error", async () => {
    mockApi.mockRejectedValue(new Error("bad"));
    expect((await linkSeries(jsonRequest("/api/anilist/series/l1/s1", { method: "POST", body: {} }), routeCtx({ libraryId: "l1", seriesId: "s1" }))).status).toBe(500);
  });

  it("DELETE unlink returns 500 on error", async () => {
    mockApi.mockRejectedValue(new Error("bad"));
    expect((await unlinkSeries(jsonRequest("/api/anilist/series/l1/s1", { method: "DELETE" }), routeCtx({ libraryId: "l1", seriesId: "s1" }))).status).toBe(500);
  });

  it("GET status proxies", async () => {
    mockApi.mockResolvedValue({ configured: true });
    expect(await (await status()).json()).toEqual({ configured: true });
  });

  it("GET sync preview proxies", async () => {
    mockApi.mockResolvedValue({ items: [] });
    expect(await (await syncPreview()).json()).toEqual({ items: [] });
  });

  it("POST sync posts an empty object", async () => {
    mockApi.mockResolvedValue({ ok: true });
    await sync();
    expect(mockApi).toHaveBeenCalledWith("/anilist/sync", { method: "POST", body: "{}" });
  });

  it("GET unlinked proxies", async () => {
    mockApi.mockResolvedValue([]);
    expect((await unlinked()).status).toBe(200);
  });
});
