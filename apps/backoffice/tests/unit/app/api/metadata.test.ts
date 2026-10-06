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
import { POST as approve } from "@/app/api/metadata/approve/route";
import { POST as reject } from "@/app/api/metadata/reject/route";
import { POST as startBatch } from "@/app/api/metadata/batch/route";
import { GET as batchReport } from "@/app/api/metadata/batch/report/route";
import { GET as batchResults } from "@/app/api/metadata/batch/results/route";
import { GET as batchMissing } from "@/app/api/metadata/missing/route";
import { GET as refreshReport } from "@/app/api/metadata/refresh/report/route";
import { POST as refresh } from "@/app/api/metadata/refresh/route";
import { POST as match } from "@/app/api/metadata/match/route";
import { POST as refreshLink } from "@/app/api/metadata/refresh-link/[id]/route";
import { GET as providers } from "@/app/api/metadata/providers/route";
import { POST as search } from "@/app/api/metadata/search/route";
import { GET as links, DELETE as deleteLink, PATCH as patchLink } from "@/app/api/metadata/links/route";

const mockApi = vi.mocked(apiFetch);

beforeEach(() => {
  revalidateTag.mockClear();
});

describe("metadata approve/reject", () => {
  it("approve strips the id and forwards the rest", async () => {
    mockApi.mockResolvedValue({ status: "ok", books_synced: 2 });
    const res = await approve(jsonRequest("/api/metadata/approve", { method: "POST", body: { id: "42", volume: 1 } }));
    expect(mockApi).toHaveBeenCalledWith("/metadata/approve/42", {
      method: "POST",
      body: JSON.stringify({ volume: 1 }),
    });
    expect(revalidateTag).toHaveBeenCalledWith("metadata", { expire: 0 });
    expect(res.status).toBe(200);
  });

  it("approve returns 500 on error", async () => {
    mockApi.mockRejectedValue(new Error("bad"));
    expect((await approve(jsonRequest("/api/metadata/approve", { method: "POST", body: { id: "1" } }))).status).toBe(500);
  });

  it("reject forwards the id", async () => {
    mockApi.mockResolvedValue({ status: "rejected" });
    const res = await reject(jsonRequest("/api/metadata/reject", { method: "POST", body: { id: "7" } }));
    expect(mockApi).toHaveBeenCalledWith("/metadata/reject/7", { method: "POST" });
    expect(revalidateTag).toHaveBeenCalledWith("metadata", { expire: 0 });
    expect(res.status).toBe(200);
  });
});

describe("metadata batch", () => {
  it("POST starts a batch", async () => {
    mockApi.mockResolvedValue({ id: "j1", status: "pending" });
    const res = await startBatch(jsonRequest("/api/metadata/batch", { method: "POST", body: { library_id: "l1" } }));
    expect(mockApi).toHaveBeenCalledWith("/metadata/batch", expect.objectContaining({ method: "POST" }));
    expect(res.status).toBe(200);
  });

  it("GET report requires an id", async () => {
    expect((await batchReport(jsonRequest("/api/metadata/batch/report"))).status).toBe(400);
  });

  it("GET report proxies", async () => {
    mockApi.mockResolvedValue({ done: true });
    const res = await batchReport(jsonRequest("/api/metadata/batch/report?id=j1"));
    expect(mockApi).toHaveBeenCalledWith("/metadata/batch/j1/report");
    expect(res.status).toBe(200);
  });

  it("GET results requires an id", async () => {
    expect((await batchResults(jsonRequest("/api/metadata/batch/results"))).status).toBe(400);
  });

  it("GET results appends the status filter", async () => {
    mockApi.mockResolvedValue([]);
    await batchResults(jsonRequest("/api/metadata/batch/results?id=j1&status=failed"));
    expect(mockApi).toHaveBeenCalledWith("/metadata/batch/j1/results?status=failed");
  });

  it("GET results omits the filter when absent", async () => {
    mockApi.mockResolvedValue([]);
    await batchResults(jsonRequest("/api/metadata/batch/results?id=j1"));
    expect(mockApi).toHaveBeenCalledWith("/metadata/batch/j1/results");
  });

  it("GET missing requires an id", async () => {
    expect((await batchMissing(jsonRequest("/api/metadata/missing"))).status).toBe(400);
  });

  it("GET missing proxies", async () => {
    mockApi.mockResolvedValue({ books: [] });
    expect((await batchMissing(jsonRequest("/api/metadata/missing?id=j1"))).status).toBe(200);
  });
});

describe("metadata refresh", () => {
  it("POST starts a refresh", async () => {
    mockApi.mockResolvedValue({ id: "j1", status: "pending" });
    expect((await refresh(jsonRequest("/api/metadata/refresh", { method: "POST", body: {} }))).status).toBe(200);
  });

  it("GET report requires job_id", async () => {
    expect((await refreshReport(jsonRequest("/api/metadata/refresh/report"))).status).toBe(400);
  });

  it("GET report proxies", async () => {
    mockApi.mockResolvedValue({ items: [] });
    await refreshReport(jsonRequest("/api/metadata/refresh/report?job_id=j1"));
    expect(mockApi).toHaveBeenCalledWith("/metadata/refresh/j1/report");
  });

  it("POST match proxies", async () => {
    mockApi.mockResolvedValue({ id: "m1" });
    expect((await match(jsonRequest("/api/metadata/match", { method: "POST", body: { series: "x" } }))).status).toBe(200);
  });

  it("POST refresh-link proxies", async () => {
    mockApi.mockResolvedValue({ refreshed: true });
    await refreshLink(jsonRequest("/api/metadata/refresh-link/m1", { method: "POST" }), routeCtx({ id: "m1" }));
    expect(mockApi).toHaveBeenCalledWith("/metadata/refresh-link/m1", { method: "POST" });
  });
});

describe("metadata providers & search & links", () => {
  it("GET providers proxies", async () => {
    mockApi.mockResolvedValue([{ id: "anilist" }]);
    expect(await (await providers()).json()).toEqual([{ id: "anilist" }]);
  });

  it("POST search proxies", async () => {
    mockApi.mockResolvedValue([]);
    expect((await search(jsonRequest("/api/metadata/search", { method: "POST", body: { q: "x" } }))).status).toBe(200);
  });

  it("GET links builds the query", async () => {
    mockApi.mockResolvedValue([]);
    await links(jsonRequest("/api/metadata/links?library_id=l1&series_name=Dune"));
    expect(mockApi).toHaveBeenCalledWith("/metadata/links?library_id=l1&series_name=Dune");
  });

  it("GET links omits empty params", async () => {
    mockApi.mockResolvedValue([]);
    await links(jsonRequest("/api/metadata/links"));
    expect(mockApi).toHaveBeenCalledWith("/metadata/links?");
  });

  it("DELETE links requires an id", async () => {
    expect((await deleteLink(jsonRequest("/api/metadata/links", { method: "DELETE" }))).status).toBe(400);
  });

  it("DELETE links proxies", async () => {
    mockApi.mockResolvedValue({ deleted: true });
    await deleteLink(jsonRequest("/api/metadata/links?id=9", { method: "DELETE" }));
    expect(mockApi).toHaveBeenCalledWith("/metadata/links/9", { method: "DELETE" });
    expect(revalidateTag).toHaveBeenCalledWith("metadata", { expire: 0 });
  });

  it("PATCH links requires an id", async () => {
    expect(
      (await patchLink(jsonRequest("/api/metadata/links", { method: "PATCH", body: { is_primary: true } }))).status,
    ).toBe(400);
  });

  it("PATCH links proxies the primary flag", async () => {
    mockApi.mockResolvedValue({ link: { id: "9" }, report: {} });
    await patchLink(
      jsonRequest("/api/metadata/links?id=9", {
        method: "PATCH",
        body: { is_primary: true, sync_series: true, sync_books: true },
      }),
    );
    expect(mockApi).toHaveBeenCalledWith("/metadata/links/9", {
      method: "PATCH",
      body: JSON.stringify({ is_primary: true, sync_series: true, sync_books: true }),
    });
    expect(revalidateTag).toHaveBeenCalledWith("metadata", { expire: 0 });
  });
});
