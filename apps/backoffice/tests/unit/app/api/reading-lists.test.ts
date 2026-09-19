// @vitest-environment node
import { describe, expect, it, vi } from "vitest";

import { jsonRequest, routeCtx } from "./helpers";

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
import { DELETE as removeSeries } from "@/app/api/reading-lists/[id]/series/[seriesId]/route";
import { PUT as reorder } from "@/app/api/reading-lists/[id]/series/reorder/route";
import { GET as memberships } from "@/app/api/reading-lists/memberships/route";

const mockApi = vi.mocked(apiFetch);

describe("/api/reading-lists", () => {
  it("GET lists", async () => {
    mockApi.mockResolvedValue([{ id: "1" }]);
    expect(await (await listLists()).json()).toEqual([{ id: "1" }]);
  });

  it("GET returns 500 on error", async () => {
    mockApi.mockRejectedValue(new Error("bad"));
    expect((await listLists()).status).toBe(500);
  });

  it("POST creates", async () => {
    mockApi.mockResolvedValue({ id: "1" });
    expect((await createList(jsonRequest("/api/reading-lists", { method: "POST", body: { name: "x" } }))).status).toBe(200);
  });

  it("GET by id", async () => {
    mockApi.mockResolvedValue({ id: "1" });
    await getList(jsonRequest("/api/reading-lists/1"), routeCtx({ id: "1" }));
    expect(mockApi).toHaveBeenCalledWith("/reading-lists/1");
  });

  it("PATCH by id forwards body", async () => {
    mockApi.mockResolvedValue({ id: "1" });
    await patchList(jsonRequest("/api/reading-lists/1", { method: "PATCH", body: { name: "y" } }), routeCtx({ id: "1" }));
    expect(mockApi).toHaveBeenCalledWith("/reading-lists/1", expect.objectContaining({ method: "PATCH" }));
  });

  it("DELETE by id returns 204", async () => {
    mockApi.mockResolvedValue(undefined);
    const res = await deleteList(jsonRequest("/api/reading-lists/1", { method: "DELETE" }), routeCtx({ id: "1" }));
    expect(res.status).toBe(204);
  });

  it("POST series returns 204", async () => {
    mockApi.mockResolvedValue(undefined);
    const res = await addSeries(jsonRequest("/api/reading-lists/1/series", { method: "POST", body: { series_id: "s1" } }), routeCtx({ id: "1" }));
    expect(mockApi).toHaveBeenCalledWith("/reading-lists/1/series", expect.objectContaining({ method: "POST" }));
    expect(res.status).toBe(204);
  });

  it("DELETE series returns 204", async () => {
    mockApi.mockResolvedValue(undefined);
    const res = await removeSeries(jsonRequest("/api/reading-lists/1/series/s1", { method: "DELETE" }), routeCtx({ id: "1", seriesId: "s1" }));
    expect(mockApi).toHaveBeenCalledWith("/reading-lists/1/series/s1", { method: "DELETE" });
    expect(res.status).toBe(204);
  });

  it("PUT reorder returns 204", async () => {
    mockApi.mockResolvedValue(undefined);
    const res = await reorder(jsonRequest("/api/reading-lists/1/series/reorder", { method: "PUT", body: { order: [1, 2] } }), routeCtx({ id: "1" }));
    expect(mockApi).toHaveBeenCalledWith("/reading-lists/1/series/reorder", expect.objectContaining({ method: "PUT" }));
    expect(res.status).toBe(204);
  });

  it("GET memberships", async () => {
    mockApi.mockResolvedValue({});
    expect((await memberships()).status).toBe(200);
  });

  it("PATCH returns 500 on error", async () => {
    mockApi.mockRejectedValue(new Error("bad"));
    expect((await patchList(jsonRequest("/api/reading-lists/1", { method: "PATCH", body: {} }), routeCtx({ id: "1" }))).status).toBe(500);
  });
});
