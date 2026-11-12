/** Browser-side discovery: external suggestions, Prowlarr feeds and hidden entries. */

import { browserFetch, browserFetchVoid, withQuery } from "./transport";

export interface DiscoverySuggestionDto {
  provider: string;
  external_id: string;
  title: string;
  authors: string[];
  description: string | null;
  genres: string[];
  cover_url: string | null;
  external_url: string | null;
  start_year: number | null;
  total_volumes: number | null;
  status: string | null;
  rating: number | null;
}

export interface ProwlarrDiscoveryItem {
  series_name: string;
  release_count: number;
  best_seeders: number;
  total_seeders: number;
  categories: string[];
  indexers: string[];
  best_release_title: string;
  best_download_url: string | null;
  best_size: number;
  best_publish_date: string | null;
  best_info_url: string | null;
  best_indexer: string | null;
  volumes_found: number[];
  local_series_id: string | null;
  local_series_name: string | null;
  volumes_already_owned: number[];
}

export interface ProwlarrDiscoveryResponse {
  items: ProwlarrDiscoveryItem[];
  all_indexers?: string[];
}

export interface HiddenDiscoveryItem {
  id: string;
  provider: string;
  external_id: string;
  title: string;
  cover_url: string | null;
}

export interface DiscoveryHideInput {
  provider: string;
  external_id: string;
  title: string;
  cover_url?: string | null;
}

export interface DiscoveryAddInput {
  library_id: string;
  provider?: string;
  external_id?: string;
  title?: string;
  description?: string | null;
  authors?: string[];
  publishers?: string[];
  genres?: string[];
  start_year?: number | null;
  total_volumes?: number | null;
  status?: string | null;
  cover_url?: string | null;
  external_url?: string | null;
  series_id?: string;
  confidence?: number;
  metadata_json?: Record<string, unknown>;
}

export const discoveryApi = {
  trending(
    provider: string,
    options: { limit?: number; period?: string; nocache?: boolean } = {}
  ): Promise<DiscoverySuggestionDto[]> {
    return browserFetch<DiscoverySuggestionDto[]>(
      withQuery("/api/discovery/trending", {
        provider,
        limit: options.limit ?? 100,
        period: options.period,
        nocache: options.nocache ? "true" : undefined,
      })
    );
  },

  prowlarr(options: {
    sort?: "seeders" | "date";
    nocache?: boolean;
    indexer?: string;
    category?: string;
  } = {}): Promise<ProwlarrDiscoveryResponse> {
    return browserFetch<ProwlarrDiscoveryResponse>(
      withQuery("/api/discovery/prowlarr", {
        sort: options.sort,
        nocache: options.nocache ? "true" : undefined,
        indexer: options.indexer,
        category: options.category,
      })
    );
  },

  hidden(): Promise<HiddenDiscoveryItem[]> {
    return browserFetch<HiddenDiscoveryItem[]>("/api/discovery/hidden");
  },

  hide(input: DiscoveryHideInput): Promise<void> {
    return browserFetchVoid("/api/discovery/hide", { method: "POST", body: input });
  },

  unhide(input: Omit<DiscoveryHideInput, "cover_url">): Promise<void> {
    return browserFetchVoid("/api/discovery/unhide", { method: "POST", body: input });
  },

  addToLibrary(input: DiscoveryAddInput): Promise<{ series_id?: string }> {
    return browserFetch<{ series_id?: string }>("/api/discovery/add-to-library", {
      method: "POST",
      body: input,
    });
  },
};
