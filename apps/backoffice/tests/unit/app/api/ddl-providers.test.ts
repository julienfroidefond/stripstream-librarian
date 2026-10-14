// @vitest-environment node
import { describe, expect, it, vi } from "vitest";

import { jsonRequest, routeCtx } from "./helpers";

vi.mock("@/lib/api", async () => {
  const { createApiMock } = await import("./helpers");
  return createApiMock();
});

import { apiFetch } from "@/lib/api";
import { GET as listProviders } from "@/app/api/ddl-providers/route";
import { POST as toggleProvider } from "@/app/api/ddl-providers/[id]/toggle/route";
import { POST as testProvider } from "@/app/api/ddl-providers/[id]/test/route";
import { POST as searchProvider } from "@/app/api/ddl-providers/[id]/search/route";
import { POST as startDownload } from "@/app/api/ddl-providers/[id]/downloads/route";
import { GET as listDownloads } from "@/app/api/ddl-provider-downloads/route";
import { DELETE as dismissDownload } from "@/app/api/ddl-provider-downloads/[id]/route";
import { POST as searchAll } from "@/app/api/ddl-providers/search-all/route";
import {
  GET as getSearchSettings,
  POST as updateSearchSettings,
} from "@/app/api/ddl-providers/search-settings/route";

const mockApi = vi.mocked(apiFetch);

describe("/api/ddl-providers", () => {
  it("GET lists providers", async () => {
    mockApi.mockResolvedValue([{ id: "fourtoutici", enabled: false }]);
    const res = await listProviders(jsonRequest("/api/ddl-providers"), routeCtx({}));
    expect(mockApi).toHaveBeenCalledWith("/ddl-providers");
    expect(res.status).toBe(200);
    await expect(res.json()).resolves.toEqual([{ id: "fourtoutici", enabled: false }]);
  });

  it("POST toggle forwards enabled + base_url", async () => {
    mockApi.mockResolvedValue([]);
    const res = await toggleProvider(
      jsonRequest("/api/ddl-providers/fourtoutici/toggle", {
        method: "POST",
        body: { enabled: true, base_url: "https://mirror.example" },
      }),
      routeCtx({ id: "fourtoutici" })
    );
    expect(mockApi).toHaveBeenCalledWith("/ddl-providers/fourtoutici/toggle", {
      method: "POST",
      body: JSON.stringify({ enabled: true, base_url: "https://mirror.example" }),
    });
    expect(res.status).toBe(200);
  });

  it("POST test tolerates an empty body", async () => {
    mockApi.mockResolvedValue({ success: true, message: "ok", result_count: 1 });
    const res = await testProvider(
      jsonRequest("/api/ddl-providers/fourtoutici/test", { method: "POST" }),
      routeCtx({ id: "fourtoutici" })
    );
    expect(mockApi).toHaveBeenCalledWith("/ddl-providers/fourtoutici/test", {
      method: "POST",
      body: "{}",
    });
    expect(res.status).toBe(200);
  });

  it("POST search forwards the query payload", async () => {
    mockApi.mockResolvedValue({ provider: "fourtoutici", query: "Astérix", results: [] });
    const res = await searchProvider(
      jsonRequest("/api/ddl-providers/fourtoutici/search", {
        method: "POST",
        body: { query: "Astérix", missing_volumes: [41] },
      }),
      routeCtx({ id: "fourtoutici" })
    );
    expect(mockApi).toHaveBeenCalledWith("/ddl-providers/fourtoutici/search", {
      method: "POST",
      body: JSON.stringify({ query: "Astérix", missing_volumes: [41] }),
    });
    expect(res.status).toBe(200);
  });

  it("POST downloads starts a download", async () => {
    mockApi.mockResolvedValue({ id: "dl-1", status: "queued" });
    const res = await startDownload(
      jsonRequest("/api/ddl-providers/fourtoutici/downloads", {
        method: "POST",
        body: { external_id: "abc", filename: "a.pdf", library_id: "lib-1", series_name: "Astérix" },
      }),
      routeCtx({ id: "fourtoutici" })
    );
    expect(mockApi).toHaveBeenCalledWith("/ddl-providers/fourtoutici/downloads", {
      method: "POST",
      body: JSON.stringify({
        external_id: "abc",
        filename: "a.pdf",
        library_id: "lib-1",
        series_name: "Astérix",
      }),
    });
    expect(res.status).toBe(200);
  });

  it("POST search-all queues a global search", async () => {
    mockApi.mockResolvedValue({ id: "job-1", status: "pending" });
    const res = await searchAll(jsonRequest("/api/ddl-providers/search-all", { method: "POST" }));
    expect(mockApi).toHaveBeenCalledWith("/ddl-providers/search-all", { method: "POST" });
    expect(res.status).toBe(200);
  });

  it("GET/POST search-settings read and update the cadence", async () => {
    mockApi.mockResolvedValue({ search_interval_minutes: 0 });
    const read = await getSearchSettings(jsonRequest("/api/ddl-providers/search-settings"));
    expect(mockApi).toHaveBeenCalledWith("/ddl-providers/search-settings");
    expect(read.status).toBe(200);

    const updated = await updateSearchSettings(
      jsonRequest("/api/ddl-providers/search-settings", {
        method: "POST",
        body: { search_interval_minutes: 60 },
      })
    );
    expect(mockApi).toHaveBeenCalledWith("/ddl-providers/search-settings", {
      method: "POST",
      body: JSON.stringify({ search_interval_minutes: 60 }),
    });
    expect(updated.status).toBe(200);
  });

  it("maps an upstream 400 on search-settings to 400", async () => {
    mockApi.mockRejectedValueOnce(new Error("API /ddl-providers/search-settings failed (400): nope"));
    const res = await updateSearchSettings(
      jsonRequest("/api/ddl-providers/search-settings", {
        method: "POST",
        body: { search_interval_minutes: -1 },
      })
    );
    expect(res.status).toBe(400);
  });

  it("maps upstream 404/400 to the same status", async () => {
    mockApi.mockRejectedValueOnce(new Error("API /x failed (404): not found"));
    const notFound = await searchProvider(
      jsonRequest("/api/ddl-providers/nope/search", { method: "POST", body: {} }),
      routeCtx({ id: "nope" })
    );
    expect(notFound.status).toBe(404);

    mockApi.mockRejectedValueOnce(new Error("API /x failed (400): bad"));
    const badRequest = await startDownload(
      jsonRequest("/api/ddl-providers/fourtoutici/downloads", { method: "POST", body: {} }),
      routeCtx({ id: "fourtoutici" })
    );
    expect(badRequest.status).toBe(400);
  });
});

describe("/api/ddl-provider-downloads", () => {
  it("GET lists downloads", async () => {
    mockApi.mockResolvedValue([{ id: "dl-1", status: "queued" }]);
    const res = await listDownloads(jsonRequest("/api/ddl-provider-downloads"), routeCtx({}));
    expect(mockApi).toHaveBeenCalledWith("/ddl-provider-downloads");
    expect(res.status).toBe(200);
  });

  it("DELETE without hard returns 204", async () => {
    mockApi.mockResolvedValue(null);
    const res = await dismissDownload(
      jsonRequest("/api/ddl-provider-downloads/dl-1", { method: "DELETE" }),
      routeCtx({ id: "dl-1" })
    );
    expect(mockApi).toHaveBeenCalledWith("/ddl-provider-downloads/dl-1", { method: "DELETE" });
    expect(res.status).toBe(204);
  });

  it("DELETE?hard=true forwards the flag and maps 404", async () => {
    mockApi.mockResolvedValue(null);
    const res = await dismissDownload(
      jsonRequest("/api/ddl-provider-downloads/dl-1?hard=true", { method: "DELETE" }),
      routeCtx({ id: "dl-1" })
    );
    expect(mockApi).toHaveBeenCalledWith("/ddl-provider-downloads/dl-1?hard=true", { method: "DELETE" });
    expect(res.status).toBe(204);

    mockApi.mockRejectedValueOnce(new Error("API /ddl-provider-downloads/x failed (404): nope"));
    const missing = await dismissDownload(
      jsonRequest("/api/ddl-provider-downloads/x", { method: "DELETE" }),
      routeCtx({ id: "x" })
    );
    expect(missing.status).toBe(404);
  });
});
