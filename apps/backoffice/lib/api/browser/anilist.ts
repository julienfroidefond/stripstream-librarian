/** Browser-side AniList status, search, linking and sync calls. */

import type {
  AnilistMediaResultDto,
  AnilistPullReportDto,
  AnilistSeriesLinkDto,
  AnilistStatusDto,
  AnilistSyncPreviewItemDto,
  AnilistSyncReportDto,
} from "../anilist";
import { browserFetch, browserFetchVoid } from "./transport";

export const anilistApi = {
  status(): Promise<AnilistStatusDto> {
    return browserFetch<AnilistStatusDto>("/api/anilist/status");
  },

  search(query: string): Promise<AnilistMediaResultDto[]> {
    return browserFetch<AnilistMediaResultDto[]>("/api/anilist/search", {
      method: "POST",
      body: { query },
    });
  },

  link(seriesId: string, anilistId: number): Promise<AnilistSeriesLinkDto> {
    return browserFetch<AnilistSeriesLinkDto>(
      `/api/series/${encodeURIComponent(seriesId)}/anilist`,
      { method: "POST", body: { anilist_id: anilistId } }
    );
  },

  unlink(seriesId: string): Promise<void> {
    return browserFetchVoid(`/api/series/${encodeURIComponent(seriesId)}/anilist`, {
      method: "DELETE",
    });
  },

  preview(): Promise<AnilistSyncPreviewItemDto[]> {
    return browserFetch<AnilistSyncPreviewItemDto[]>("/api/anilist/sync/preview");
  },

  sync(): Promise<AnilistSyncReportDto> {
    return browserFetch<AnilistSyncReportDto>("/api/anilist/sync", { method: "POST" });
  },

  pull(): Promise<AnilistPullReportDto> {
    return browserFetch<AnilistPullReportDto>("/api/anilist/pull", { method: "POST" });
  },
};
