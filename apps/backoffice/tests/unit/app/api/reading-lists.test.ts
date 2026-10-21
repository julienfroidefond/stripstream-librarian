// @vitest-environment node
import { beforeEach, describe, expect, it, vi } from "vitest";

import { jsonRequest, routeCtx } from "./helpers";

const revalidateTag = vi.hoisted(() => vi.fn());
vi.mock("next/cache", () => ({ revalidateTag }));

vi.mock("@/lib/api", async () => {
  const { createApiMock } = await import("./helpers");
  return createApiMock();
});

import { apiFetch } from "@/lib/api";
import { GET as listLists, POST as createList } from "@/app/api/reading-lists/route";
import {
  GET as getList,
  PATCH as patchList,
  DELETE as deleteList,
} from "@/app/api/reading-lists/[id]/route";
import { POST as addSeries } from "@/app/api/reading-lists/[id]/series/route";
import { DELETE as removeSeries, POST as moveSeries } from "@/app/api/reading-lists/[id]/series/[seriesId]/route";
import { PUT as reorder } from "@/app/api/reading-lists/[id]/series/reorder/route";
import { GET as memberships } from "@/app/api/reading-lists/memberships/route";
import { POST as suggestLists } from "@/app/api/reading-lists/ai-suggest/route";

const mockApi = vi.mocked(apiFetch);

beforeEach(() => {
  revalidateTag.mockClear();
});

describe("/api/reading-lists", () => {
  it("GET lists", async () => {
    mockApi.mockResolvedValue([{ id: "1" }]);
    expect(await (await listLists()).json()).toEqual([{ id: "1" }]);
    expect(revalidateTag).not.toHaveBeenCalled();
  });

  it("GET returns 500 on error", async () => {
    mockApi.mockRejectedValue(new Error("bad"));
    expect((await listLists()).status).toBe(500);
    expect(revalidateTag).not.toHaveBeenCalled();
  });

  it("POST creates and invalidates the list cache", async () => {
    mockApi.mockResolvedValue({ id: "1" });
    expect((await createList(jsonRequest("/api/reading-lists", { method: "POST", body: { name: "x" } }))).status).toBe(200);
    expect(revalidateTag).toHaveBeenCalledWith("reading-lists", { expire: 0 });
  });

  it("POST returns 500 on error without invalidating the cache", async () => {
    mockApi.mockRejectedValue(new Error("bad"));
    const res = await createList(jsonRequest("/api/reading-lists", { method: "POST", body: { name: "x" } }));
    expect(res.status).toBe(500);
    expect(revalidateTag).not.toHaveBeenCalled();
  });

  it("GET by id", async () => {
    mockApi.mockResolvedValue({ id: "1" });
    await getList(jsonRequest("/api/reading-lists/1"), routeCtx({ id: "1" }));
    expect(mockApi).toHaveBeenCalledWith("/reading-lists/1");
    expect(revalidateTag).not.toHaveBeenCalled();
  });

  it("PATCH by id forwards body and invalidates the list cache", async () => {
    mockApi.mockResolvedValue({ id: "1" });
    await patchList(jsonRequest("/api/reading-lists/1", { method: "PATCH", body: { name: "y" } }), routeCtx({ id: "1" }));
    expect(mockApi).toHaveBeenCalledWith("/reading-lists/1", {
      method: "PATCH",
      body: JSON.stringify({ name: "y" }),
    });
    expect(revalidateTag).toHaveBeenCalledWith("reading-lists", { expire: 0 });
  });

  it("DELETE by id returns 204 and invalidates the list cache", async () => {
    mockApi.mockResolvedValue(undefined);
    const res = await deleteList(jsonRequest("/api/reading-lists/1", { method: "DELETE" }), routeCtx({ id: "1" }));
    expect(res.status).toBe(204);
    expect(revalidateTag).toHaveBeenCalledWith("reading-lists", { expire: 0 });
  });

  it("POST series returns 204 and invalidates the list cache", async () => {
    mockApi.mockResolvedValue(undefined);
    const res = await addSeries(jsonRequest("/api/reading-lists/1/series", { method: "POST", body: { series_id: "s1" } }), routeCtx({ id: "1" }));
    expect(mockApi).toHaveBeenCalledWith("/reading-lists/1/series", expect.objectContaining({ method: "POST" }));
    expect(res.status).toBe(204);
    expect(revalidateTag).toHaveBeenCalledWith("reading-lists", { expire: 0 });
  });

  it("DELETE series returns 204 and invalidates the list cache", async () => {
    mockApi.mockResolvedValue(undefined);
    const res = await removeSeries(jsonRequest("/api/reading-lists/1/series/s1", { method: "DELETE" }), routeCtx({ id: "1", seriesId: "s1" }));
    expect(mockApi).toHaveBeenCalledWith("/reading-lists/1/series/s1", { method: "DELETE" });
    expect(res.status).toBe(204);
    expect(revalidateTag).toHaveBeenCalledWith("reading-lists", { expire: 0 });
  });

  it("POST move returns 204 and invalidates the list cache", async () => {
    mockApi.mockResolvedValue(undefined);
    const res = await moveSeries(
      jsonRequest("/api/reading-lists/1/series/s1/move", {
        method: "POST",
        body: { to_list_id: "2" },
      }),
      routeCtx({ id: "1", seriesId: "s1" })
    );
    expect(mockApi).toHaveBeenCalledWith("/reading-lists/1/series/s1/move", {
      method: "POST",
      body: JSON.stringify({ to_list_id: "2" }),
    });
    expect(res.status).toBe(204);
    expect(revalidateTag).toHaveBeenCalledWith("reading-lists", { expire: 0 });
  });

  it("POST move returns 500 on error without invalidating the cache", async () => {
    mockApi.mockRejectedValue(new Error("bad"));
    const res = await moveSeries(
      jsonRequest("/api/reading-lists/1/series/s1/move", {
        method: "POST",
        body: { to_list_id: "2" },
      }),
      routeCtx({ id: "1", seriesId: "s1" })
    );
    expect(res.status).toBe(500);
    expect(revalidateTag).not.toHaveBeenCalled();
  });

  it("POST ai-suggest returns moves alongside the other proposals", async () => {
    const payload = {
      additions: [],
      moves: [
        {
          from_list_id: "1",
          from_list_name: "Favourites",
          to_list_id: "2",
          to_list_name: "Space operas",
          reason: "wrong list",
          series: [],
        },
      ],
      new_lists: [],
    };
    mockApi.mockResolvedValue(payload);
    const res = await suggestLists(
      jsonRequest("/api/reading-lists/ai-suggest", { method: "POST", body: {} })
    );
    expect(await res.json()).toEqual(payload);
  });

  it("PUT reorder returns 204 and invalidates the list cache", async () => {
    mockApi.mockResolvedValue(undefined);
    const res = await reorder(jsonRequest("/api/reading-lists/1/series/reorder", { method: "PUT", body: { order: [1, 2] } }), routeCtx({ id: "1" }));
    expect(mockApi).toHaveBeenCalledWith("/reading-lists/1/series/reorder", expect.objectContaining({ method: "PUT" }));
    expect(res.status).toBe(204);
    expect(revalidateTag).toHaveBeenCalledWith("reading-lists", { expire: 0 });
  });

  it("GET memberships", async () => {
    mockApi.mockResolvedValue({});
    expect((await memberships()).status).toBe(200);
    expect(revalidateTag).not.toHaveBeenCalled();
  });

  it("POST ai-suggest forwards the request and returns the proposals", async () => {
    mockApi.mockResolvedValue({ additions: [], new_lists: [] });
    const res = await suggestLists(
      jsonRequest("/api/reading-lists/ai-suggest", { method: "POST", body: { limit: 80 } })
    );
    expect(mockApi).toHaveBeenCalledWith(
      "/reading-lists/ai-suggest",
      expect.objectContaining({ method: "POST", body: JSON.stringify({ limit: 80 }) })
    );
    expect(res.status).toBe(200);
    expect(await res.json()).toEqual({ additions: [], new_lists: [] });
    expect(revalidateTag).not.toHaveBeenCalled();
  });

  it("POST ai-suggest returns 500 on error", async () => {
    mockApi.mockRejectedValue(new Error("AI disabled"));
    expect(
      (await suggestLists(jsonRequest("/api/reading-lists/ai-suggest", { method: "POST", body: {} }))).status
    ).toBe(500);
    expect(revalidateTag).not.toHaveBeenCalled();
  });

  it("POST ai-suggest forwards a 422 from the API", async () => {
    mockApi.mockRejectedValue(
      new Error('API /reading-lists/ai-suggest failed (422): {"error":"AI disabled"}')
    );
    expect(
      (await suggestLists(jsonRequest("/api/reading-lists/ai-suggest", { method: "POST", body: {} }))).status
    ).toBe(422);
    expect(revalidateTag).not.toHaveBeenCalled();
  });

  it("does not invalidate the cache when adding a duplicate series fails", async () => {
    mockApi.mockRejectedValue(
      new Error('API /reading-lists/1/series failed (400): {"error":"series already in reading list"}')
    );
    const res = await addSeries(
      jsonRequest("/api/reading-lists/1/series", { method: "POST", body: { series_id: "s1" } }),
      routeCtx({ id: "1" })
    );
    expect(res.status).toBe(500);
    expect(revalidateTag).not.toHaveBeenCalled();
  });

  it("PATCH returns 500 on error without invalidating the cache", async () => {
    mockApi.mockRejectedValue(new Error("bad"));
    expect((await patchList(jsonRequest("/api/reading-lists/1", { method: "PATCH", body: {} }), routeCtx({ id: "1" }))).status).toBe(500);
    expect(revalidateTag).not.toHaveBeenCalled();
  });

  it("does not invalidate the cache when other mutations fail", async () => {
    mockApi.mockRejectedValue(new Error("bad"));
    const responses = await Promise.all([
      deleteList(jsonRequest("/api/reading-lists/1", { method: "DELETE" }), routeCtx({ id: "1" })),
      addSeries(jsonRequest("/api/reading-lists/1/series", { method: "POST", body: { series_id: "s1" } }), routeCtx({ id: "1" })),
      removeSeries(jsonRequest("/api/reading-lists/1/series/s1", { method: "DELETE" }), routeCtx({ id: "1", seriesId: "s1" })),
      reorder(jsonRequest("/api/reading-lists/1/series/reorder", { method: "PUT", body: { order: [1, 2] } }), routeCtx({ id: "1" })),
    ]);
    expect(responses.map((response) => response.status)).toEqual([500, 500, 500, 500]);
    expect(revalidateTag).not.toHaveBeenCalled();
  });
});
