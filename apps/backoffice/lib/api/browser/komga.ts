/** Browser-side Komga read-status synchronisation. */

import type {
  KomgaSyncReportSummary,
  KomgaSyncRequest,
  KomgaSyncResponse,
} from "../komga";
import { browserFetch } from "./transport";

export const komgaApi = {
  reports(): Promise<KomgaSyncReportSummary[]> {
    return browserFetch<KomgaSyncReportSummary[]>("/api/komga/reports");
  },

  report(id: string): Promise<KomgaSyncResponse> {
    return browserFetch<KomgaSyncResponse>(`/api/komga/reports/${encodeURIComponent(id)}`);
  },

  sync(request: KomgaSyncRequest): Promise<KomgaSyncResponse> {
    return browserFetch<KomgaSyncResponse>("/api/komga/sync", {
      method: "POST",
      body: request,
    });
  },
};
