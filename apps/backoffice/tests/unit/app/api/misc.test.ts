// @vitest-environment node
import { describe, expect, it, vi } from "vitest";

import { jsonRequest, routeCtx } from "./helpers";

vi.mock("@/lib/api", async () => {
  const { createApiMock } = await import("./helpers");
  return createApiMock();
});

import {
  apiFetch,
  fetchStats,
  getKomgaReport,
  listFolders,
  listKomgaReports,
} from "@/lib/api";
import { GET as komgaReports } from "@/app/api/komga/reports/route";
import { GET as komgaReport } from "@/app/api/komga/reports/[id]/route";
import { POST as komgaSync } from "@/app/api/komga/sync/route";
import { POST as prowlarrSearch } from "@/app/api/prowlarr/search/route";
import { GET as prowlarrTest } from "@/app/api/prowlarr/test/route";
import { POST as qbAdd } from "@/app/api/qbittorrent/add/route";
import { GET as qbTest } from "@/app/api/qbittorrent/test/route";
import { GET as telegramTest } from "@/app/api/telegram/test/route";
import { GET as torrents } from "@/app/api/torrent-downloads/route";
import { DELETE as deleteTorrent } from "@/app/api/torrent-downloads/[id]/route";
import { POST as retryTorrent } from "@/app/api/torrent-downloads/[id]/retry/route";
import { GET as listBlacklist, POST as addBlacklist } from "@/app/api/release-blacklist/route";
import { DELETE as deleteBlacklist } from "@/app/api/release-blacklist/[id]/route";
import { DELETE as deleteAvailable } from "@/app/api/available-downloads/[id]/route";
import { GET as folders } from "@/app/api/folders/route";
import { GET as stats } from "@/app/api/stats/route";

const mockApi = vi.mocked(apiFetch);

describe("komga routes", () => {
  it("GET reports delegates to listKomgaReports", async () => {
    vi.mocked(listKomgaReports).mockResolvedValue([]);
    expect((await komgaReports()).status).toBe(200);
  });

  it("GET report delegates to getKomgaReport", async () => {
    vi.mocked(getKomgaReport).mockResolvedValue({ id: "r1" } as never);
    await komgaReport(jsonRequest("/api/komga/reports/r1"), routeCtx({ id: "r1" }));
    expect(getKomgaReport).toHaveBeenCalledWith("r1");
  });

  it("POST sync forwards body", async () => {
    mockApi.mockResolvedValue({ ok: true });
    expect((await komgaSync(jsonRequest("/api/komga/sync", { method: "POST", body: { full: true } }))).status).toBe(200);
  });
});

describe("connection test routes", () => {
  it("GET prowlarr test", async () => {
    mockApi.mockResolvedValue({ ok: true });
    await prowlarrTest();
    expect(mockApi).toHaveBeenCalledWith("/prowlarr/test");
  });

  it("GET qbittorrent test", async () => {
    mockApi.mockResolvedValue({ ok: true });
    await qbTest();
    expect(mockApi).toHaveBeenCalledWith("/qbittorrent/test");
  });

  it("GET telegram test", async () => {
    mockApi.mockResolvedValue({ ok: true });
    await telegramTest();
    expect(mockApi).toHaveBeenCalledWith("/telegram/test");
  });

  it("GET test returns 500 on error", async () => {
    mockApi.mockRejectedValue(new Error("bad"));
    expect((await prowlarrTest()).status).toBe(500);
  });
});

describe("download routes", () => {
  it("POST prowlarr search forwards body", async () => {
    mockApi.mockResolvedValue([]);
    expect((await prowlarrSearch(jsonRequest("/api/prowlarr/search", { method: "POST", body: { query: "dune" } }))).status).toBe(200);
  });

  it("POST qbittorrent add forwards body", async () => {
    mockApi.mockResolvedValue({ ok: true });
    expect((await qbAdd(jsonRequest("/api/qbittorrent/add", { method: "POST", body: { url: "magnet:?x" } }))).status).toBe(200);
  });

  it("GET torrent downloads", async () => {
    mockApi.mockResolvedValue([]);
    await torrents();
    expect(mockApi).toHaveBeenCalledWith("/torrent-downloads");
  });

  it("DELETE torrent", async () => {
    mockApi.mockResolvedValue({ deleted: true });
    await deleteTorrent(jsonRequest("/api/torrent-downloads/t1", { method: "DELETE" }), routeCtx({ id: "t1" }));
    expect(mockApi).toHaveBeenCalledWith("/torrent-downloads/t1", { method: "DELETE" });
  });

  it("POST retry torrent", async () => {
    mockApi.mockResolvedValue({ ok: true });
    await retryTorrent(jsonRequest("/api/torrent-downloads/t1/retry", { method: "POST" }), routeCtx({ id: "t1" }));
    expect(mockApi).toHaveBeenCalledWith("/torrent-downloads/t1/retry", { method: "POST" });
  });

  it("DELETE available download forwards query", async () => {
    mockApi.mockResolvedValue({ deleted: true });
    await deleteAvailable(jsonRequest("/api/available-downloads/d1?permanent=1", { method: "DELETE" }), routeCtx({ id: "d1" }));
    expect(mockApi).toHaveBeenCalledWith("/available-downloads/d1?permanent=1", { method: "DELETE" });
  });
});

describe("release blacklist routes", () => {
  it("GET list", async () => {
    mockApi.mockResolvedValue([]);
    expect((await listBlacklist()).status).toBe(200);
  });

  it("POST add forwards body", async () => {
    mockApi.mockResolvedValue({ ok: true });
    expect((await addBlacklist(jsonRequest("/api/release-blacklist", { method: "POST", body: { title: "x" } }))).status).toBe(200);
  });

  it("DELETE entry", async () => {
    mockApi.mockResolvedValue({ deleted: true });
    await deleteBlacklist(jsonRequest("/api/release-blacklist/b1", { method: "DELETE" }), routeCtx({ id: "b1" }));
    expect(mockApi).toHaveBeenCalledWith("/release-blacklist/b1", { method: "DELETE" });
  });
});

describe("folders & stats", () => {
  it("GET folders without path", async () => {
    vi.mocked(listFolders).mockResolvedValue([]);
    await folders(jsonRequest("/api/folders"));
    expect(listFolders).toHaveBeenCalledWith(undefined);
  });

  it("GET folders with path", async () => {
    vi.mocked(listFolders).mockResolvedValue([]);
    await folders(jsonRequest("/api/folders?path=%2Fdata"));
    expect(listFolders).toHaveBeenCalledWith("/data");
  });

  it("GET folders returns 500 on error", async () => {
    vi.mocked(listFolders).mockRejectedValue(new Error("bad"));
    expect((await folders(jsonRequest("/api/folders"))).status).toBe(500);
  });

  it("GET stats defaults to week", async () => {
    vi.mocked(fetchStats).mockResolvedValue({ total: 1 } as never);
    await stats(jsonRequest("/api/stats"));
    expect(fetchStats).toHaveBeenCalledWith("week");
  });

  it("GET stats accepts day and month", async () => {
    vi.mocked(fetchStats).mockResolvedValue({ total: 1 } as never);
    await stats(jsonRequest("/api/stats?period=day"));
    expect(fetchStats).toHaveBeenCalledWith("day");
    await stats(jsonRequest("/api/stats?period=month"));
    expect(fetchStats).toHaveBeenCalledWith("month");
  });

  it("GET stats falls back to week for unknown period", async () => {
    vi.mocked(fetchStats).mockResolvedValue({ total: 1 } as never);
    await stats(jsonRequest("/api/stats?period=year"));
    expect(fetchStats).toHaveBeenCalledWith("week");
  });

  it("GET stats returns 500 on error", async () => {
    vi.mocked(fetchStats).mockRejectedValue(new Error("bad"));
    expect((await stats(jsonRequest("/api/stats"))).status).toBe(500);
  });
});
