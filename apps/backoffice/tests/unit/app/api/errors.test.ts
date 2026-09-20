// @vitest-environment node
import { beforeEach, describe, expect, it, vi } from "vitest";

import { jsonRequest, routeCtx } from "./helpers";

vi.mock("@/lib/api", async () => {
  const { createApiMock } = await import("./helpers");
  return createApiMock();
});
vi.mock("next/cache", () => ({ revalidatePath: vi.fn(), revalidateTag: vi.fn() }));

import * as api from "@/lib/api";

import { GET as listAnilistLinks } from "@/app/api/anilist/links/route";
import { POST as anilistPull } from "@/app/api/anilist/pull/route";
import { POST as anilistSearch } from "@/app/api/anilist/search/route";
import {
  GET as anilistSeries,
  POST as anilistLink,
  DELETE as anilistUnlink,
} from "@/app/api/anilist/series/[libraryId]/[seriesId]/route";
import { GET as anilistStatus } from "@/app/api/anilist/status/route";
import { GET as anilistSyncPreview } from "@/app/api/anilist/sync/preview/route";
import { POST as anilistSync } from "@/app/api/anilist/sync/route";
import { GET as anilistUnlinked } from "@/app/api/anilist/unlinked/route";
import { DELETE as deleteAvailable } from "@/app/api/available-downloads/[id]/route";
import { GET as latestFound } from "@/app/api/download-detection/latest-found/route";
import { GET as discoveryHidden } from "@/app/api/discovery/hidden/route";
import { POST as discoveryHide } from "@/app/api/discovery/hide/route";
import { GET as discoveryTrending } from "@/app/api/discovery/trending/route";
import { POST as discoveryUnhide } from "@/app/api/discovery/unhide/route";
import { POST as genresAiSuggest } from "@/app/api/genres/ai-suggest/route";
import { POST as genresAssign } from "@/app/api/genres/assign/route";
import { GET as genresUntagged } from "@/app/api/genres/untagged-series/route";
import { PATCH as genrePatch, DELETE as genreDelete } from "@/app/api/genres/[name]/route";
import { GET as jobsActive } from "@/app/api/jobs/active/route";
import { GET as jobGet } from "@/app/api/jobs/[id]/route";
import { POST as jobCancel } from "@/app/api/jobs/[id]/cancel/route";
import { GET as komgaReports } from "@/app/api/komga/reports/route";
import { GET as komgaReport } from "@/app/api/komga/reports/[id]/route";
import { POST as komgaSync } from "@/app/api/komga/sync/route";
import { PATCH as libraryMetadataProvider } from "@/app/api/libraries/[id]/metadata-provider/route";
import { PATCH as libraryReadingStatus } from "@/app/api/libraries/[id]/reading-status-provider/route";
import { PATCH as libraryTags } from "@/app/api/libraries/[id]/tags/route";
import { GET as librarySeriesMetadata } from "@/app/api/libraries/[id]/series/[seriesId]/metadata/route";
import { DELETE as librarySeriesDelete } from "@/app/api/libraries/[id]/series/[seriesId]/route";
import { POST as metadataBatch } from "@/app/api/metadata/batch/route";
import { GET as metadataBatchReport } from "@/app/api/metadata/batch/report/route";
import { GET as metadataBatchResults } from "@/app/api/metadata/batch/results/route";
import { GET as metadataMissing } from "@/app/api/metadata/missing/route";
import { POST as metadataMatch } from "@/app/api/metadata/match/route";
import { GET as metadataProviders } from "@/app/api/metadata/providers/route";
import { GET as metadataRefreshReport } from "@/app/api/metadata/refresh/report/route";
import { POST as metadataRefresh } from "@/app/api/metadata/refresh/route";
import { POST as metadataRefreshLink } from "@/app/api/metadata/refresh-link/[id]/route";
import { POST as metadataReject } from "@/app/api/metadata/reject/route";
import { POST as metadataSearch } from "@/app/api/metadata/search/route";
import { GET as metadataLinks, DELETE as metadataLinkDelete } from "@/app/api/metadata/links/route";
import { POST as prowlarrSearch } from "@/app/api/prowlarr/search/route";
import { POST as qbAdd } from "@/app/api/qbittorrent/add/route";
import { GET as qbTest } from "@/app/api/qbittorrent/test/route";
import { GET as readingListMemberships } from "@/app/api/reading-lists/memberships/route";
import { GET as readingLists, POST as readingListCreate } from "@/app/api/reading-lists/route";
import {
  GET as readingListGet,
  DELETE as readingListDelete,
} from "@/app/api/reading-lists/[id]/route";
import { POST as readingListAddSeries } from "@/app/api/reading-lists/[id]/series/route";
import { DELETE as readingListRemoveSeries } from "@/app/api/reading-lists/[id]/series/[seriesId]/route";
import { PUT as readingListReorder } from "@/app/api/reading-lists/[id]/series/reorder/route";
import { GET as blacklist, POST as blacklistAdd } from "@/app/api/release-blacklist/route";
import { DELETE as blacklistDelete } from "@/app/api/release-blacklist/[id]/route";
import { GET as archivedSeries } from "@/app/api/series/archived/route";
import { GET as archivedSeriesOne } from "@/app/api/series/archived/[id]/route";
import { POST as seriesCreate } from "@/app/api/series/create/route";
import { GET as seriesGenres } from "@/app/api/series/genres/route";
import { POST as seriesMarkRead } from "@/app/api/series/mark-read/route";
import { GET as seriesProviderStatuses } from "@/app/api/series/provider-statuses/route";
import { GET as seriesStatuses } from "@/app/api/series/statuses/route";
import { GET as seriesSearch } from "@/app/api/series/search/route";
import { POST as seriesMerge } from "@/app/api/series/[seriesId]/merge/route";
import { DELETE as seriesRatingDelete } from "@/app/api/series/[seriesId]/rating/route";
import { POST as seriesRenameBooks } from "@/app/api/series/[seriesId]/rename-books/route";
import { POST as aiTaggingTest } from "@/app/api/settings/ai_tagging/test/route";
import { GET as statusMappings, POST as statusMappingCreate } from "@/app/api/settings/status-mappings/route";
import { DELETE as statusMappingDelete } from "@/app/api/settings/status-mappings/[id]/route";
import { GET as telegramTest } from "@/app/api/telegram/test/route";
import { GET as torrents } from "@/app/api/torrent-downloads/route";
import { DELETE as torrentDelete } from "@/app/api/torrent-downloads/[id]/route";
import { POST as torrentRetry } from "@/app/api/torrent-downloads/[id]/retry/route";
import { GET as folders } from "@/app/api/folders/route";

beforeEach(() => {
  for (const value of Object.values(api)) {
    if (typeof value === "function" && "mockRejectedValue" in value) {
      (value as ReturnType<typeof vi.fn>).mockRejectedValue(new Error("boom"));
    }
  }
});

type Case = [string, () => Promise<Response>, number?];

const req = (path: string, method = "GET", body?: unknown) =>
  jsonRequest(path, { method, body });

const cases: Case[] = [
  ["anilist links", () => listAnilistLinks()],
  ["anilist pull", () => anilistPull()],
  ["anilist search", () => anilistSearch(req("/api/anilist/search", "POST", {}))],
  ["anilist status", () => anilistStatus()],
  ["anilist sync preview", () => anilistSyncPreview()],
  ["anilist sync", () => anilistSync()],
  ["anilist unlinked", () => anilistUnlinked()],
  ["anilist series GET", () => anilistSeries(req("/api/anilist/series/l1/s1"), routeCtx({ libraryId: "l1", seriesId: "s1" })), 404],
  ["anilist series link", () => anilistLink(req("/api/anilist/series/l1/s1", "POST", {}), routeCtx({ libraryId: "l1", seriesId: "s1" }))],
  ["anilist series unlink", () => anilistUnlink(req("/api/anilist/series/l1/s1", "DELETE"), routeCtx({ libraryId: "l1", seriesId: "s1" }))],
  ["available-downloads delete", () => deleteAvailable(req("/api/available-downloads/d1", "DELETE"), routeCtx({ id: "d1" }))],
  ["download-detection latest-found", () => latestFound()],
  ["discovery hidden", () => discoveryHidden()],
  ["discovery hide", () => discoveryHide(req("/api/discovery/hide", "POST", {}))],
  ["discovery trending", () => discoveryTrending(req("/api/discovery/trending"))],
  ["discovery unhide", () => discoveryUnhide(req("/api/discovery/unhide", "POST", {}))],
  ["genres ai-suggest", () => genresAiSuggest(req("/api/genres/ai-suggest", "POST", {}))],
  ["genres assign", () => genresAssign(req("/api/genres/assign", "POST", {}))],
  ["genres untagged-series", () => genresUntagged(req("/api/genres/untagged-series"))],
  ["genre patch", () => genrePatch(req("/api/genres/x", "PATCH", {}), routeCtx({ name: "x" }))],
  ["genre delete", () => genreDelete(req("/api/genres/x", "DELETE"), routeCtx({ name: "x" }))],
  ["jobs active", () => jobsActive()],
  ["job get", () => jobGet(req("/api/jobs/j1"), routeCtx({ id: "j1" }))],
  ["job cancel", () => jobCancel(req("/api/jobs/j1/cancel", "POST"), routeCtx({ id: "j1" }))],
  ["komga reports", () => komgaReports()],
  ["komga report", () => komgaReport(req("/api/komga/reports/r1"), routeCtx({ id: "r1" }))],
  ["komga sync", () => komgaSync(req("/api/komga/sync", "POST", {}))],
  ["library metadata-provider", () => libraryMetadataProvider(req("/api/libraries/l1/metadata-provider", "PATCH", {}), routeCtx({ id: "l1" }))],
  ["library reading-status-provider", () => libraryReadingStatus(req("/api/libraries/l1/reading-status-provider", "PATCH", {}), routeCtx({ id: "l1" }))],
  ["library tags", () => libraryTags(req("/api/libraries/l1/tags", "PATCH", {}), routeCtx({ id: "l1" }))],
  ["library series metadata", () => librarySeriesMetadata(req("/api/libraries/l1/series/s1/metadata"), routeCtx({ id: "l1", seriesId: "s1" }))],
  ["library series delete", () => librarySeriesDelete(req("/api/libraries/l1/series/s1", "DELETE"), routeCtx({ id: "l1", seriesId: "s1" }))],
  ["metadata batch", () => metadataBatch(req("/api/metadata/batch", "POST", {}))],
  ["metadata batch report", () => metadataBatchReport(req("/api/metadata/batch/report?id=j1"))],
  ["metadata batch results", () => metadataBatchResults(req("/api/metadata/batch/results?id=j1"))],
  ["metadata missing", () => metadataMissing(req("/api/metadata/missing?id=j1"))],
  ["metadata match", () => metadataMatch(req("/api/metadata/match", "POST", {}))],
  ["metadata providers", () => metadataProviders()],
  ["metadata refresh report", () => metadataRefreshReport(req("/api/metadata/refresh/report?job_id=j1"))],
  ["metadata refresh", () => metadataRefresh(req("/api/metadata/refresh", "POST", {}))],
  ["metadata refresh-link", () => metadataRefreshLink(req("/api/metadata/refresh-link/m1", "POST"), routeCtx({ id: "m1" }))],
  ["metadata reject", () => metadataReject(req("/api/metadata/reject", "POST", { id: "1" }))],
  ["metadata search", () => metadataSearch(req("/api/metadata/search", "POST", {}))],
  ["metadata links", () => metadataLinks(req("/api/metadata/links"))],
  ["metadata link delete", () => metadataLinkDelete(req("/api/metadata/links?id=1", "DELETE"))],
  ["prowlarr search", () => prowlarrSearch(req("/api/prowlarr/search", "POST", {}))],
  ["qbittorrent add", () => qbAdd(req("/api/qbittorrent/add", "POST", {}))],
  ["qbittorrent test", () => qbTest()],
  ["reading list memberships", () => readingListMemberships()],
  ["reading lists", () => readingLists()],
  ["reading list create", () => readingListCreate(req("/api/reading-lists", "POST", {}))],
  ["reading list get", () => readingListGet(req("/api/reading-lists/1"), routeCtx({ id: "1" }))],
  ["reading list delete", () => readingListDelete(req("/api/reading-lists/1", "DELETE"), routeCtx({ id: "1" }))],
  ["reading list add series", () => readingListAddSeries(req("/api/reading-lists/1/series", "POST", {}), routeCtx({ id: "1" }))],
  ["reading list remove series", () => readingListRemoveSeries(req("/api/reading-lists/1/series/s1", "DELETE"), routeCtx({ id: "1", seriesId: "s1" }))],
  ["reading list reorder", () => readingListReorder(req("/api/reading-lists/1/series/reorder", "PUT", {}), routeCtx({ id: "1" }))],
  ["release blacklist", () => blacklist()],
  ["release blacklist add", () => blacklistAdd(req("/api/release-blacklist", "POST", {}))],
  ["release blacklist delete", () => blacklistDelete(req("/api/release-blacklist/b1", "DELETE"), routeCtx({ id: "b1" }))],
  ["series archived", () => archivedSeries()],
  ["series archived one", () => archivedSeriesOne(req("/api/series/archived/s1"), routeCtx({ id: "s1" }))],
  ["series create", () => seriesCreate(req("/api/series/create", "POST", {}))],
  ["series genres", () => seriesGenres()],
  ["series mark-read", () => seriesMarkRead(req("/api/series/mark-read", "POST", { series: [] }))],
  ["series provider statuses", () => seriesProviderStatuses()],
  ["series statuses", () => seriesStatuses()],
  ["series search", () => seriesSearch(req("/api/series/search"))],
  ["series merge", () => seriesMerge(req("/api/series/s1/merge", "POST", {}), routeCtx({ seriesId: "s1" }))],
  ["series rating delete", () => seriesRatingDelete(req("/api/series/s1/rating", "DELETE"), routeCtx({ seriesId: "s1" }))],
  ["series rename books", () => seriesRenameBooks(req("/api/series/s1/rename-books", "POST", {}), routeCtx({ seriesId: "s1" }))],
  ["settings ai tagging test", () => aiTaggingTest(req("/api/settings/ai_tagging/test", "POST", {}))],
  ["settings status mappings", () => statusMappings()],
  ["settings status mapping create", () => statusMappingCreate(req("/api/settings/status-mappings", "POST", {}))],
  ["settings status mapping delete", () => statusMappingDelete(req("/api/settings/status-mappings/1", "DELETE"), routeCtx({ id: "1" }))],
  ["telegram test", () => telegramTest()],
  ["torrent downloads", () => torrents()],
  ["torrent delete", () => torrentDelete(req("/api/torrent-downloads/t1", "DELETE"), routeCtx({ id: "t1" }))],
  ["torrent retry", () => torrentRetry(req("/api/torrent-downloads/t1/retry", "POST"), routeCtx({ id: "t1" }))],
  ["folders", () => folders(req("/api/folders"))],
];

describe("route handler error paths", () => {
  it.each(cases)("%s returns an error response", async (_name, call, expected = 500) => {
    const res = await call();
    expect(res.status).toBe(expected);
    const body = await res.json();
    if (expected >= 400) {
      expect(body).toHaveProperty("error");
    } else {
      expect(body).toEqual([]);
    }
  });
});
