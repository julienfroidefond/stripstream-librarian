/** Browser-side job list and job actions. */

import type { IndexJobDto } from "../jobs";
import { browserFetch, browserFetchVoid } from "./transport";

export const jobsApi = {
  list(): Promise<IndexJobDto[]> {
    return browserFetch<IndexJobDto[]>("/api/jobs/list");
  },

  cancel(id: string): Promise<void> {
    return browserFetchVoid(`/api/jobs/${encodeURIComponent(id)}/cancel`, { method: "POST" });
  },

  replay(id: string): Promise<unknown> {
    return browserFetch<unknown>(`/api/jobs/${encodeURIComponent(id)}/replay`, { method: "POST" });
  },
};
