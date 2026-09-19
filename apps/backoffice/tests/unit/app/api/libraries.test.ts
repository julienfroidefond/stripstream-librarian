// @vitest-environment node
import { describe, expect, it, vi } from "vitest";

import { jsonRequest, routeCtx } from "./helpers";

vi.mock("@/lib/api", async () => {
  const { createApiMock } = await import("./helpers");
  return createApiMock();
});
vi.mock("next/cache", () => ({ revalidatePath: vi.fn(), revalidateTag: vi.fn() }));

import { apiFetch, deleteSeries, fetchSeriesMetadata, updateLibraryMonitoring, updateSeries } from "@/lib/api";
import { PATCH as patchMetadataProvider } from "@/app/api/libraries/[id]/metadata-provider/route";
import { PATCH as patchMonitoring } from "@/app/api/libraries/[id]/monitoring/route";
import { PATCH as patchReadingStatus } from "@/app/api/libraries/[id]/reading-status-provider/route";
import { PATCH as patchTags } from "@/app/api/libraries/[id]/tags/route";
import { GET as getSeriesMetadata } from "@/app/api/libraries/[id]/series/[seriesId]/metadata/route";
import {
  PATCH as patchSeries,
  DELETE as deleteLibrarySeries,
} from "@/app/api/libraries/[id]/series/[seriesId]/route";

const mockApi = vi.mocked(apiFetch);

describe("/api/libraries/[id]", () => {
  it("PATCH metadata provider", async () => {
    mockApi.mockResolvedValue({ id: "l1" });
    await patchMetadataProvider(jsonRequest("/api/libraries/l1/metadata-provider", { method: "PATCH", body: { provider: "anilist" } }), routeCtx({ id: "l1" }));
    expect(mockApi).toHaveBeenCalledWith("/libraries/l1/metadata-provider", expect.objectContaining({ method: "PATCH" }));
  });

  it("PATCH monitoring delegates with all fields", async () => {
    vi.mocked(updateLibraryMonitoring).mockResolvedValue({ id: "l1" } as never);
    const res = await patchMonitoring(
      jsonRequest("/api/libraries/l1/monitoring", {
        method: "PATCH",
        body: {
          monitor_enabled: true,
          scan_mode: "watcher",
          watcher_enabled: true,
          metadata_refresh_mode: "auto",
          download_detection_mode: "off",
        },
      }),
      routeCtx({ id: "l1" })
    );
    expect(updateLibraryMonitoring).toHaveBeenCalledWith("l1", true, "watcher", true, "auto", "off");
    expect(res.status).toBe(200);
  });

  it("PATCH monitoring returns 500 on error", async () => {
    vi.mocked(updateLibraryMonitoring).mockRejectedValue(new Error("bad"));
    expect(
      (await patchMonitoring(jsonRequest("/api/libraries/l1/monitoring", { method: "PATCH", body: {} }), routeCtx({ id: "l1" }))).status
    ).toBe(500);
  });

  it("PATCH reading status provider", async () => {
    mockApi.mockResolvedValue({ id: "l1" });
    await patchReadingStatus(jsonRequest("/api/libraries/l1/reading-status-provider", { method: "PATCH", body: { provider: "komga" } }), routeCtx({ id: "l1" }));
    expect(mockApi).toHaveBeenCalledWith("/libraries/l1/reading-status-provider", expect.objectContaining({ method: "PATCH" }));
  });

  it("PATCH tags", async () => {
    mockApi.mockResolvedValue({ ok: true });
    await patchTags(jsonRequest("/api/libraries/l1/tags", { method: "PATCH", body: { tags: [] } }), routeCtx({ id: "l1" }));
    expect(mockApi).toHaveBeenCalledWith("/libraries/l1/tags", expect.objectContaining({ method: "PATCH" }));
  });
});

describe("/api/libraries/[id]/series/[seriesId]", () => {
  it("GET series metadata delegates to fetchSeriesMetadata", async () => {
    vi.mocked(fetchSeriesMetadata).mockResolvedValue({ links: [] } as never);
    await getSeriesMetadata(jsonRequest("/api/libraries/l1/series/s1/metadata"), routeCtx({ id: "l1", seriesId: "s1" }));
    expect(fetchSeriesMetadata).toHaveBeenCalledWith("s1");
  });

  it("PATCH series delegates to updateSeries", async () => {
    vi.mocked(updateSeries).mockResolvedValue({ id: "s1" } as never);
    await patchSeries(jsonRequest("/api/libraries/l1/series/s1", { method: "PATCH", body: { name: "x" } }), routeCtx({ id: "l1", seriesId: "s1" }));
    expect(updateSeries).toHaveBeenCalledWith("s1", { name: "x" });
  });

  it("PATCH series returns 500 on error", async () => {
    vi.mocked(updateSeries).mockRejectedValue(new Error("bad"));
    expect((await patchSeries(jsonRequest("/api/libraries/l1/series/s1", { method: "PATCH", body: {} }), routeCtx({ id: "l1", seriesId: "s1" }))).status).toBe(500);
  });

  it("DELETE series returns deleted true", async () => {
    vi.mocked(deleteSeries).mockResolvedValue(undefined);
    const res = await deleteLibrarySeries(jsonRequest("/api/libraries/l1/series/s1", { method: "DELETE" }), routeCtx({ id: "l1", seriesId: "s1" }));
    expect(deleteSeries).toHaveBeenCalledWith("s1");
    expect(await res.json()).toEqual({ deleted: true });
  });
});
