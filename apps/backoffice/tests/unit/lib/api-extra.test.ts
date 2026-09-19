import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const cookieStore = vi.hoisted(() => ({ get: vi.fn() }));

vi.mock("next/headers", () => ({
  cookies: async () => cookieStore,
}));

import {
  addSeriesToReadingList,
  addTelegramSource,
  approveMetadataMatch,
  createMetadataMatch,
  createReadingList,
  deleteMetadataLink,
  deleteReadingList,
  deleteSeries,
  deleteTelegramSource,
  disconnectTelegram,
  dismissTelegramBook,
  downloadTelegramBook,
  fetchMetadataProviders,
  fetchReadingList,
  fetchReadingLists,
  fetchAllGenres,
  fetchSeriesMetadata,
  fetchSeriesRatings,
  fetchSeriesMemberships,
  fetchSeriesReadingLists,
  fetchTelegramAuthorized,
  fetchTelegramAvailable,
  fetchTelegramDownloads,
  fetchTorrentDownloads,
  getArchivedSeries,
  getDownloadDetectionReport,
  getDownloadDetectionResults,
  getKomgaReport,
  getMetadataBatchReport,
  getMetadataBatchResults,
  getMetadataLink,
  getMetadataRefreshReport,
  getMissingBooks,
  getReadingOverview,
  getReadingStatusLink,
  getReadingStatusMatchReport,
  getReadingStatusMatchResults,
  getReadingStatusPushReport,
  getReadingStatusPushResults,
  getTelegramMonitorStatus,
  listArchivedSeries,
  listKomgaReports,
  listTelegramBooks,
  listTelegramSources,
  rejectMetadataMatch,
  removeSeriesFromReadingList,
  reorderReadingListSeries,
  saveTelegramMonitorSettings,
  searchMetadata,
  startDownloadDetection,
  startMetadataBatch,
  startMetadataRefresh,
  startMetadataRefreshAll,
  startReadingStatusMatch,
  startReadingStatusPull,
  startReadingStatusPush,
  startRssPoll,
  startTelegramAuth,
  startTelegramSync,
  startTelegramSyncIncremental,
  syncTelegramSources,
  updateLibraryMetadataProvider,
  updateReadingList,
  updateSeries,
  verifyTelegramCode,
} from "@/lib/api";

function makeResponse(body: unknown, status = 200): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    text: async () => (typeof body === "string" ? body : JSON.stringify(body)),
    json: async () => body,
  } as unknown as Response;
}

let fetchMock: ReturnType<typeof vi.fn>;

beforeEach(() => {
  process.env.API_BASE_URL = "http://api:7080";
  process.env.API_BOOTSTRAP_TOKEN = "bootstrap-token";
  cookieStore.get.mockReset();
  cookieStore.get.mockReturnValue(undefined);
  fetchMock = vi.fn().mockResolvedValue(makeResponse({ ok: true }));
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

type Call = {
  url: string;
  method: string;
  body: unknown;
  init: RequestInit & { next?: unknown };
};

function lastCall(): Call {
  const calls = fetchMock.mock.calls;
  const [rawUrl, init] = calls[calls.length - 1] as [string, RequestInit];
  return {
    url: rawUrl.replace(/^https?:\/\/[^/]+/, ""),
    method: init.method ?? "GET",
    body: init.body ? JSON.parse(init.body as string) : undefined,
    init,
  };
}

describe("metadata links", () => {
  it("searches with and without a provider", async () => {
    await searchMetadata("lib-1", "Berserk");
    expect(lastCall()).toMatchObject({ url: "/metadata/search", method: "POST" });
    expect(lastCall().body).toEqual({ library_id: "lib-1", series_name: "Berserk" });

    await searchMetadata("lib-1", "Berserk", "bdfugue");
    expect(lastCall().body).toEqual({
      library_id: "lib-1",
      series_name: "Berserk",
      provider: "bdfugue",
    });
  });

  it("lists providers and creates a match", async () => {
    await fetchMetadataProviders();
    expect(lastCall()).toMatchObject({ url: "/metadata/providers", method: "GET" });

    const payload = {
      library_id: "lib-1",
      series_name: "Berserk",
      provider: "bdfugue",
      external_id: "ext-1",
      title: "Berserk",
      metadata_json: { a: 1 },
    };
    await createMetadataMatch(payload);
    expect(lastCall().url).toBe("/metadata/match");
    expect(lastCall().body).toEqual(payload);
  });

  it("approves and rejects matches", async () => {
    await approveMetadataMatch("m-1", true, false);
    expect(lastCall()).toMatchObject({ url: "/metadata/approve/m-1", method: "POST" });
    expect(lastCall().body).toEqual({ sync_series: true, sync_books: false });

    await rejectMetadataMatch("m-1");
    expect(lastCall()).toMatchObject({ url: "/metadata/reject/m-1", method: "POST" });
    expect(lastCall().body).toBeUndefined();
  });

  it("fetches links with the series tag cache options", async () => {
    await getMetadataLink("series-1");
    const call = lastCall();
    expect(call.url).toBe("/metadata/links?series_id=series-1");
    expect((call.init.next as { tags: string[] }).tags).toEqual([
      "metadata",
      "series:series-1",
    ]);
  });

  it("fetches the reading-status link with a 60s revalidate", async () => {
    await getReadingStatusLink("series-1");
    const call = lastCall();
    expect(call.url).toBe("/series/series-1/anilist");
    expect((call.init.next as { revalidate: number }).revalidate).toBe(60);
  });

  it("fetches missing books per link", async () => {
    await getMissingBooks("link-1");
    const call = lastCall();
    expect(call.url).toBe("/metadata/missing/link-1");
    expect((call.init.next as { tags: string[] }).tags).toContain("link:link-1");
  });

  it("deletes a metadata link", async () => {
    await deleteMetadataLink("link-1");
    expect(lastCall()).toMatchObject({ url: "/metadata/links/link-1", method: "DELETE" });
  });

  it("patches the library metadata provider with its fallback", async () => {
    await updateLibraryMetadataProvider("lib-1", "bdfugue", "bdphile");
    expect(lastCall()).toMatchObject({
      url: "/libraries/lib-1/metadata-provider",
      method: "PATCH",
    });
    expect(lastCall().body).toEqual({
      metadata_provider: "bdfugue",
      fallback_metadata_provider: "bdphile",
    });
  });
});

describe("komga reports", () => {
  it("lists reports and fetches one by id", async () => {
    await listKomgaReports();
    expect(lastCall()).toMatchObject({ url: "/komga/reports", method: "GET" });

    await getKomgaReport("job-1");
    expect(lastCall()).toMatchObject({ url: "/komga/reports/job-1", method: "GET" });
  });
});

describe("metadata batch jobs", () => {
  it("starts a batch with an empty payload by default", async () => {
    await startMetadataBatch();
    expect(lastCall()).toMatchObject({ url: "/metadata/batch", method: "POST" });
    expect(lastCall().body).toEqual({});
  });

  it("starts a forced batch for one library", async () => {
    await startMetadataBatch("lib-1", true);
    expect(lastCall().body).toEqual({ library_id: "lib-1", force_rematch: true });
  });

  it("starts the refresh and refresh-all variants", async () => {
    await startMetadataRefresh("lib-1");
    expect(lastCall()).toMatchObject({ url: "/metadata/refresh", method: "POST" });
    expect(lastCall().body).toEqual({ library_id: "lib-1" });

    await startMetadataRefreshAll();
    expect(lastCall()).toMatchObject({ url: "/metadata/refresh-all", method: "POST" });
    expect(lastCall().body).toEqual({});
  });

  it("starts the reading-status match and push", async () => {
    await startReadingStatusMatch("lib-1");
    expect(lastCall()).toMatchObject({ url: "/reading-status/match", method: "POST" });
    expect(lastCall().body).toEqual({ library_id: "lib-1" });

    await startReadingStatusPush();
    expect(lastCall()).toMatchObject({ url: "/reading-status/push", method: "POST" });
    expect(lastCall().body).toEqual({});
  });

  it("starts a ratings pull without a body", async () => {
    await startReadingStatusPull();
    expect(lastCall()).toMatchObject({ url: "/ratings/pull", method: "POST" });
    expect(lastCall().body).toBeUndefined();
  });

  it("fetches match reports and results", async () => {
    await getReadingStatusMatchReport("job-1");
    expect(lastCall().url).toBe("/reading-status/match/job-1/report");

    await getReadingStatusMatchResults("job-1");
    expect(lastCall().url).toBe("/reading-status/match/job-1/results");

    await getReadingStatusPushReport("job-1");
    expect(lastCall().url).toBe("/reading-status/push/job-1/report");

    await getReadingStatusPushResults("job-1");
    expect(lastCall().url).toBe("/reading-status/push/job-1/results");
  });

  it("starts the download detection and rss poll", async () => {
    await startDownloadDetection("lib-1");
    expect(lastCall()).toMatchObject({ url: "/download-detection/start", method: "POST" });
    expect(lastCall().body).toEqual({ library_id: "lib-1" });

    await startRssPoll();
    expect(lastCall()).toMatchObject({ url: "/prowlarr-rss/start", method: "POST" });
    expect(lastCall().body).toEqual({});
  });

  it("fetches the download detection report and filtered results", async () => {
    await getDownloadDetectionReport("job-1");
    expect(lastCall().url).toBe("/download-detection/job-1/report");

    await getDownloadDetectionResults("job-1");
    expect(lastCall().url).toBe("/download-detection/job-1/results");

    await getDownloadDetectionResults("job-1", "found");
    expect(lastCall().url).toBe("/download-detection/job-1/results?status=found");
  });

  it("fetches the metadata refresh and batch reports", async () => {
    await getMetadataRefreshReport("job-1");
    expect(lastCall().url).toBe("/metadata/refresh/job-1/report");

    await getMetadataBatchReport("job-1");
    expect(lastCall().url).toBe("/metadata/batch/job-1/report");

    await getMetadataBatchResults("job-1");
    expect(lastCall().url).toBe("/metadata/batch/job-1/results");

    await getMetadataBatchResults("job-1", "auto_matched");
    expect(lastCall().url).toBe("/metadata/batch/job-1/results?status=auto_matched");
  });
});

describe("series mutations", () => {
  it("updates and deletes a series", async () => {
    await updateSeries("series-1", { name: "New" } as never);
    expect(lastCall()).toMatchObject({ url: "/series/series-1", method: "PATCH" });

    await deleteSeries("series-1");
    expect(lastCall()).toMatchObject({ url: "/series/series-1", method: "DELETE" });
  });

  it("fetches the metadata and ratings with cache tags", async () => {
    await fetchSeriesMetadata("series-1");
    const metadataCall = lastCall();
    expect(metadataCall.url).toBe("/series/series-1/metadata");
    expect((metadataCall.init.next as { tags: string[] }).tags).toContain("series:series-1");

    await fetchSeriesRatings("series-1");
    const ratingsCall = lastCall();
    expect(ratingsCall.url).toBe("/series/series-1/ratings");
    expect((ratingsCall.init.next as { revalidate: number }).revalidate).toBe(30);
  });

  it("returns an empty genre list when the request fails", async () => {
    fetchMock.mockRejectedValueOnce(new Error("boom"));
    await expect(fetchAllGenres()).resolves.toEqual([]);
    expect(lastCall().url).toBe("/series/genres");
  });
});

describe("torrents", () => {
  it("lists the torrent downloads", async () => {
    await fetchTorrentDownloads();
    expect(lastCall()).toMatchObject({ url: "/torrent-downloads", method: "GET" });
  });
});

describe("reading lists", () => {
  it("lists every list and the ones for a series", async () => {
    await fetchReadingLists();
    const call = lastCall();
    expect(call.url).toBe("/reading-lists");
    expect((call.init.next as { revalidate: number }).revalidate).toBe(30);

    await fetchSeriesReadingLists("series-1");
    const seriesCall = lastCall();
    expect(seriesCall.url).toBe("/reading-lists?series_id=series-1");
    expect((seriesCall.init.next as { tags: string[] }).tags).toContain("series:series-1");
  });

  it("lists memberships and a single list", async () => {
    await fetchSeriesMemberships();
    expect(lastCall().url).toBe("/reading-lists/memberships");

    await fetchReadingList("list-1");
    expect(lastCall().url).toBe("/reading-lists/list-1");
  });

  it("creates, updates and deletes a list", async () => {
    await createReadingList("Favourites", "top picks");
    expect(lastCall()).toMatchObject({ url: "/reading-lists", method: "POST" });
    expect(lastCall().body).toEqual({ name: "Favourites", description: "top picks" });

    await updateReadingList("list-1", { name: "Renamed" });
    expect(lastCall()).toMatchObject({ url: "/reading-lists/list-1", method: "PATCH" });
    expect(lastCall().body).toEqual({ name: "Renamed" });

    await deleteReadingList("list-1");
    expect(lastCall()).toMatchObject({ url: "/reading-lists/list-1", method: "DELETE" });
  });

  it("adds, removes and reorders series", async () => {
    await addSeriesToReadingList("list-1", "series-1");
    expect(lastCall()).toMatchObject({ url: "/reading-lists/list-1/series", method: "POST" });
    expect(lastCall().body).toEqual({ series_id: "series-1" });

    await removeSeriesFromReadingList("list-1", "series-1");
    expect(lastCall()).toMatchObject({
      url: "/reading-lists/list-1/series/series-1",
      method: "DELETE",
    });

    await reorderReadingListSeries("list-1", ["a", "b"]);
    expect(lastCall()).toMatchObject({
      url: "/reading-lists/list-1/series/reorder",
      method: "PUT",
    });
    expect(lastCall().body).toEqual({ series_ids: ["a", "b"] });
  });
});

describe("archive and reading overview", () => {
  it("lists archived series and fetches one", async () => {
    await listArchivedSeries();
    expect(lastCall()).toMatchObject({ url: "/admin/series/archived", method: "GET" });

    await getArchivedSeries("arch-1");
    expect(lastCall()).toMatchObject({
      url: "/admin/series/archived/arch-1",
      method: "GET",
    });
  });

  it("fetches the reading overview", async () => {
    await getReadingOverview();
    expect(lastCall()).toMatchObject({ url: "/admin/reading-overview", method: "GET" });
  });
});

describe("telegram monitor", () => {
  it("reads the status and saves the settings", async () => {
    await getTelegramMonitorStatus();
    expect(lastCall()).toMatchObject({ url: "/telegram-monitor/status", method: "GET" });

    const settings = { api_id: 123, api_hash: "hash", phone: "+33" };
    await saveTelegramMonitorSettings(settings);
    expect(lastCall()).toMatchObject({ url: "/telegram-monitor/settings", method: "POST" });
    expect(lastCall().body).toEqual(settings);
  });

  it("runs the auth lifecycle", async () => {
    await startTelegramAuth();
    expect(lastCall()).toMatchObject({ url: "/telegram-monitor/auth/start", method: "POST" });

    await verifyTelegramCode("12345");
    expect(lastCall()).toMatchObject({ url: "/telegram-monitor/auth/verify", method: "POST" });
    expect(lastCall().body).toEqual({ code: "12345" });

    await disconnectTelegram();
    expect(lastCall()).toMatchObject({ url: "/telegram-monitor/auth", method: "DELETE" });
  });

  it("manages the sources", async () => {
    await listTelegramSources();
    expect(lastCall()).toMatchObject({ url: "/telegram-monitor/sources", method: "GET" });

    await addTelegramSource({ channel_username: "@chan", library_id: "lib-1" });
    expect(lastCall()).toMatchObject({ url: "/telegram-monitor/sources", method: "POST" });
    expect(lastCall().body).toEqual({ channel_username: "@chan", library_id: "lib-1" });

    await deleteTelegramSource("src-1");
    expect(lastCall()).toMatchObject({
      url: "/telegram-monitor/sources/src-1",
      method: "DELETE",
    });

    await syncTelegramSources();
    expect(lastCall()).toMatchObject({ url: "/telegram-monitor/sync", method: "POST" });
  });

  it("starts the full and incremental syncs", async () => {
    await startTelegramSync();
    expect(lastCall()).toMatchObject({ url: "/telegram-monitor/start", method: "POST" });

    await startTelegramSyncIncremental();
    expect(lastCall()).toMatchObject({
      url: "/telegram-monitor/start-incremental",
      method: "POST",
    });
  });

  it("reports authorization from the status", async () => {
    fetchMock.mockResolvedValueOnce(makeResponse({ authorized: true }));
    await expect(fetchTelegramAuthorized()).resolves.toBe(true);

    fetchMock.mockRejectedValueOnce(new Error("down"));
    await expect(fetchTelegramAuthorized()).resolves.toBe(false);
  });

  it("lists books and downloads or dismisses them", async () => {
    await listTelegramBooks();
    expect(lastCall()).toMatchObject({ url: "/telegram-monitor/books", method: "GET" });

    await downloadTelegramBook("book-1", "lib-1");
    expect(lastCall()).toMatchObject({
      url: "/telegram-monitor/books/book-1/download",
      method: "POST",
    });
    expect(lastCall().body).toEqual({ library_id: "lib-1" });

    await downloadTelegramBook("book-1");
    expect(lastCall().body).toEqual({ library_id: null });

    await dismissTelegramBook("book-1");
    expect(lastCall()).toMatchObject({ url: "/telegram-monitor/books/book-1", method: "DELETE" });
  });

  it("fetches the available groups and the download queue", async () => {
    await fetchTelegramAvailable();
    expect(lastCall()).toMatchObject({ url: "/telegram-monitor/available", method: "GET" });

    await fetchTelegramDownloads();
    expect(lastCall()).toMatchObject({ url: "/telegram-monitor/downloads", method: "GET" });
  });
});
