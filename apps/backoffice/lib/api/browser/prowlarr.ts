/** Browser-side Prowlarr release search. */

import type { ProwlarrSearchResponse } from "../downloads";
import { browserFetch } from "./transport";

export interface ProwlarrSearchInput {
  series_name: string;
  custom_query: string;
  missing_volumes?: Array<{ volume_number: number | null; title: string | null }>;
}

export const prowlarrApi = {
  search(input: ProwlarrSearchInput): Promise<ProwlarrSearchResponse & { error?: string }> {
    return browserFetch<ProwlarrSearchResponse & { error?: string }>("/api/prowlarr/search", {
      method: "POST",
      body: input,
    });
  },
};
