// @vitest-environment node
import { describe, expect, it, vi } from "vitest";

import { jsonRequest, routeCtx } from "./helpers";

vi.mock("@/lib/api", async () => {
  const { createApiMock } = await import("./helpers");
  return createApiMock();
});

import {
  apiFetch,
  cancelJob,
  listJobs,
  rebuildIndex,
  rebuildThumbnails,
  regenerateThumbnails,
  startDownloadDetection,
  startMetadataBatch,
  startMetadataRefresh,
  startMetadataRefreshAll,
  startReadingStatusMatch,
  startReadingStatusPull,
  startReadingStatusPush,
  startRssPoll,
} from "@/lib/api";
import { GET as getJob } from "@/app/api/jobs/[id]/route";
import { POST as cancel } from "@/app/api/jobs/[id]/cancel/route";
import { GET as events } from "@/app/api/jobs/[id]/events/route";
import { POST as replay } from "@/app/api/jobs/[id]/replay/route";
import { GET as active } from "@/app/api/jobs/active/route";
import { GET as list } from "@/app/api/jobs/list/route";

const mockApi = vi.mocked(apiFetch);

describe("/api/jobs", () => {
  it("GET active proxies", async () => {
    mockApi.mockResolvedValue([]);
    await active();
    expect(mockApi).toHaveBeenCalledWith("/index/jobs/active");
  });

  it("GET list delegates to listJobs", async () => {
    vi.mocked(listJobs).mockResolvedValue({ items: [] } as never);
    expect((await list()).status).toBe(200);
  });

  it("GET list returns 500 on error", async () => {
    vi.mocked(listJobs).mockRejectedValue(new Error("bad"));
    expect((await list()).status).toBe(500);
  });

  it("GET by id proxies", async () => {
    mockApi.mockResolvedValue({ id: "j1", type: "scan" });
    await getJob(jsonRequest("/api/jobs/j1"), routeCtx({ id: "j1" }));
    expect(mockApi).toHaveBeenCalledWith("/index/jobs/j1");
  });

  it("POST cancel delegates to cancelJob", async () => {
    vi.mocked(cancelJob).mockResolvedValue({ ok: true } as never);
    await cancel(jsonRequest("/api/jobs/j1/cancel", { method: "POST" }), routeCtx({ id: "j1" }));
    expect(cancelJob).toHaveBeenCalledWith("j1");
  });

  it("GET events forwards supported filters", async () => {
    mockApi.mockResolvedValue([]);
    await events(jsonRequest("/api/jobs/j1/events?level=error&event_type=scan&limit=50"), routeCtx({ id: "j1" }));
    expect(mockApi).toHaveBeenCalledWith("/index/jobs/j1/events?level=error&event_type=scan&limit=50");
  });

  it("GET events omits absent filters", async () => {
    mockApi.mockResolvedValue([]);
    await events(jsonRequest("/api/jobs/j1/events"), routeCtx({ id: "j1" }));
    expect(mockApi).toHaveBeenCalledWith("/index/jobs/j1/events");
  });

  it("GET events returns 500 on error", async () => {
    mockApi.mockRejectedValue(new Error("bad"));
    expect((await events(jsonRequest("/api/jobs/j1/events"), routeCtx({ id: "j1" }))).status).toBe(500);
  });
});

describe("POST /api/jobs/[id]/replay", () => {
  const fns = {
    rebuildIndex: vi.mocked(rebuildIndex),
    rebuildThumbnails: vi.mocked(rebuildThumbnails),
    regenerateThumbnails: vi.mocked(regenerateThumbnails),
    startMetadataBatch: vi.mocked(startMetadataBatch),
    startMetadataRefresh: vi.mocked(startMetadataRefresh),
    startMetadataRefreshAll: vi.mocked(startMetadataRefreshAll),
    startReadingStatusMatch: vi.mocked(startReadingStatusMatch),
    startReadingStatusPush: vi.mocked(startReadingStatusPush),
    startReadingStatusPull: vi.mocked(startReadingStatusPull),
    startDownloadDetection: vi.mocked(startDownloadDetection),
    startRssPoll: vi.mocked(startRssPoll),
  };

  it.each([
    ["rebuild", "rebuildIndex", ["l1"]],
    ["full_rebuild", "rebuildIndex", ["l1", true]],
    ["rescan", "rebuildIndex", ["l1", false, true]],
    ["scan", "rebuildIndex", ["l1"]],
    ["thumbnail_rebuild", "rebuildThumbnails", ["l1"]],
    ["thumbnail_regenerate", "regenerateThumbnails", ["l1"]],
    ["metadata_batch", "startMetadataBatch", ["l1"]],
    ["metadata_refresh", "startMetadataRefresh", ["l1"]],
    ["metadata_refresh_all", "startMetadataRefreshAll", ["l1"]],
    ["reading_status_match", "startReadingStatusMatch", ["l1"]],
    ["reading_status_push", "startReadingStatusPush", ["l1"]],
    ["rating_pull", "startReadingStatusPull", []],
    ["download_detection", "startDownloadDetection", ["l1"]],
    ["prowlarr_rss", "startRssPoll", ["l1"]],
  ])("replays %s via %s", async (type, fn, args) => {
    mockApi.mockResolvedValue({ id: "j1", type, library_id: "l1" });
    fns[fn as keyof typeof fns].mockResolvedValue({ id: "new" } as never);
    const res = await replay(jsonRequest("/api/jobs/j1/replay", { method: "POST" }), routeCtx({ id: "j1" }));
    expect(fns[fn as keyof typeof fns]).toHaveBeenCalledWith(...(args as never[]));
    expect(res.status).toBe(200);
  });

  it("rejects replaying an unknown type", async () => {
    mockApi.mockResolvedValue({ id: "j1", type: "unknown_type", library_id: "l1" });
    const res = await replay(jsonRequest("/api/jobs/j1/replay", { method: "POST" }), routeCtx({ id: "j1" }));
    expect(res.status).toBe(400);
    expect(await res.json()).toEqual({ error: "Cannot replay job type: unknown_type" });
  });

  it("requires a library id for metadata batch", async () => {
    mockApi.mockResolvedValue({ id: "j1", type: "metadata_batch", library_id: null });
    const res = await replay(jsonRequest("/api/jobs/j1/replay", { method: "POST" }), routeCtx({ id: "j1" }));
    expect(res.status).toBe(400);
  });

  it("returns 500 when the job lookup fails", async () => {
    mockApi.mockRejectedValue(new Error("bad"));
    expect((await replay(jsonRequest("/api/jobs/j1/replay", { method: "POST" }), routeCtx({ id: "j1" }))).status).toBe(500);
  });
});
