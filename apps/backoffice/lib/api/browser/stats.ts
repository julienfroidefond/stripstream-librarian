/** Browser-side dashboard statistics and per-user reading overview. */

import type { StatsResponse } from "../stats";
import type { UserReadingOverviewSeriesBookDto } from "../users";
import { browserFetch, withQuery } from "./transport";

export type StatsPeriod = "day" | "week" | "month";

export const statsApi = {
  get(period?: StatsPeriod): Promise<StatsResponse> {
    const path = period && period !== "week" ? withQuery("/api/stats", { period }) : "/api/stats";
    return browserFetch<StatsResponse>(path);
  },

  readingOverviewSeriesBooks(
    userId: string,
    seriesId?: string
  ): Promise<UserReadingOverviewSeriesBookDto[]> {
    return browserFetch<UserReadingOverviewSeriesBookDto[]>(
      withQuery(`/api/reading-overview/${encodeURIComponent(userId)}/series-books`, {
        series_id: seriesId,
      })
    );
  },
};
