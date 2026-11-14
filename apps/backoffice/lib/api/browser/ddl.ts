/** Browser-side direct-download (DDL) providers and download history. */

import type {
  DdlProviderDownloadDto,
  DdlProviderDto,
  DdlProviderSearchRequest,
  DdlProviderSearchResponse,
  DdlProviderTestResponse,
  StartDdlProviderDownloadRequest,
} from "../ddl-providers";
import { browserFetch, browserFetchVoid, withQuery } from "./transport";

export type DdlSearchSettings = { search_interval_minutes: number };

export const ddlApi = {
  providers(): Promise<DdlProviderDto[]> {
    return browserFetch<DdlProviderDto[]>("/api/ddl-providers");
  },

  toggle(id: string, enabled: boolean, baseUrl?: string): Promise<DdlProviderDto[]> {
    return browserFetch<DdlProviderDto[]>(
      `/api/ddl-providers/${encodeURIComponent(id)}/toggle`,
      { method: "POST", body: { enabled, base_url: baseUrl } }
    );
  },

  test(id: string, query?: string): Promise<DdlProviderTestResponse> {
    return browserFetch<DdlProviderTestResponse>(
      `/api/ddl-providers/${encodeURIComponent(id)}/test`,
      { method: "POST", body: { query } }
    );
  },

  search(
    id: string,
    body: DdlProviderSearchRequest,
    options: { signal?: AbortSignal } = {}
  ): Promise<DdlProviderSearchResponse> {
    return browserFetch<DdlProviderSearchResponse>(
      `/api/ddl-providers/${encodeURIComponent(id)}/search`,
      { method: "POST", body, signal: options.signal }
    );
  },

  startDownload(
    id: string,
    body: StartDdlProviderDownloadRequest
  ): Promise<{ id: string; status: string }> {
    return browserFetch<{ id: string; status: string }>(
      `/api/ddl-providers/${encodeURIComponent(id)}/downloads`,
      { method: "POST", body }
    );
  },

  searchSettings(): Promise<DdlSearchSettings> {
    return browserFetch<DdlSearchSettings>("/api/ddl-providers/search-settings");
  },

  updateSearchSettings(searchIntervalMinutes: number): Promise<DdlSearchSettings> {
    return browserFetch<DdlSearchSettings>("/api/ddl-providers/search-settings", {
      method: "POST",
      body: { search_interval_minutes: searchIntervalMinutes },
    });
  },

  searchAll(): Promise<{ id: string; status: string }> {
    return browserFetch<{ id: string; status: string }>("/api/ddl-providers/search-all", {
      method: "POST",
    });
  },

  downloads(): Promise<DdlProviderDownloadDto[]> {
    return browserFetch<DdlProviderDownloadDto[]>("/api/ddl-provider-downloads");
  },

  dismissDownload(id: string, hard = false): Promise<void> {
    return browserFetchVoid(
      withQuery(`/api/ddl-provider-downloads/${encodeURIComponent(id)}`, {
        hard: hard ? "true" : undefined,
      }),
      { method: "DELETE" }
    );
  },
};
