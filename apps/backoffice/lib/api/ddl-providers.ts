/**
 * Fournisseurs de téléchargement direct (catalogues JSON/HTML externes).
 *
 * Contrairement à Prowlarr/qBittorrent, le fichier est téléchargé puis importé
 * automatiquement dans la bibliothèque (même comportement que le moniteur Telegram).
 */

import { apiFetch } from "./client";

export type DdlProviderDto = {
  id: string;
  label: string;
  /** `json` pour une API native, `html` pour une page à scraper. */
  kind: string;
  description: string;
  base_url: string;
  requires_auth: boolean;
  enabled: boolean;
};

export type DdlProviderSearchResultDto = {
  provider: string;
  external_id: string;
  filename: string;
  size: number | null;
  series_name: string | null;
  volume_number: number | null;
  all_volumes: number[];
  matched_missing_volumes: number[];
  match_confidence: string | null;
  /** Le fichier a été reconnu comme appartenant à la série demandée. */
  series_matched: boolean;
  already_owned: boolean;
  download_id: string | null;
  download_status: string | null;
};

export type DdlProviderSearchResponse = {
  provider: string;
  query: string;
  results: DdlProviderSearchResultDto[];
};

export type DdlProviderTestResponse = {
  success: boolean;
  message: string;
  result_count: number | null;
};

export type DdlProviderDownloadStatus = "queued" | "downloading" | "imported" | "failed" | "dismissed";

export type DdlProviderDownloadDto = {
  id: string;
  provider: string;
  external_id: string;
  filename: string;
  file_size: number | null;
  bytes_downloaded: number;
  series_name: string | null;
  series_id: string | null;
  volume_number: number | null;
  library_id: string | null;
  library_name: string | null;
  status: DdlProviderDownloadStatus;
  error_message: string | null;
  created_at: string;
  updated_at: string;
};

export type DdlProviderSearchRequest = {
  query?: string;
  series_name?: string;
  library_id?: string;
  missing_volumes?: number[];
  limit?: number;
};

export type StartDdlProviderDownloadRequest = {
  external_id: string;
  filename: string;
  size?: number | null;
  library_id: string;
  series_name: string;
  volume_number?: number | null;
};

/** Every known DDL provider with its enabled state. */
export async function fetchDdlProviders(): Promise<DdlProviderDto[]> {
  return apiFetch<DdlProviderDto[]>("/ddl-providers");
}

/** True when at least one DDL provider is enabled (gates the search action in toolbars). */
export async function hasEnabledDdlProvider(): Promise<boolean> {
  return apiFetch<DdlProviderDto[]>("/ddl-providers", { next: { revalidate: 60 } })
    .then((list) => list.some((p) => p.enabled))
    .catch(() => false);
}

/** Enable/disable a DDL provider and, optionally, override its base URL. */
export async function toggleDdlProvider(
  id: string,
  enabled: boolean,
  baseUrl?: string
): Promise<DdlProviderDto[]> {
  return apiFetch<DdlProviderDto[]>(
    `/ddl-providers/${encodeURIComponent(id)}/toggle`,
    {
      method: "POST",
      body: JSON.stringify({ enabled, base_url: baseUrl }),
    }
  );
}

/** Reachability / result-count probe. */
export async function testDdlProvider(
  id: string,
  query?: string
): Promise<DdlProviderTestResponse> {
  return apiFetch<DdlProviderTestResponse>(
    `/ddl-providers/${encodeURIComponent(id)}/test`,
    { method: "POST", body: JSON.stringify({ query }) }
  );
}

/** Search a DDL provider, ranked against the target series' missing volumes. */
export async function searchDdlProvider(
  id: string,
  body: DdlProviderSearchRequest
): Promise<DdlProviderSearchResponse> {
  return apiFetch<DdlProviderSearchResponse>(
    `/ddl-providers/${encodeURIComponent(id)}/search`,
    { method: "POST", body: JSON.stringify(body) }
  );
}

/** Queue a file for download + automatic import. */
export async function startDdlProviderDownload(
  id: string,
  body: StartDdlProviderDownloadRequest
): Promise<{ id: string; status: string }> {
  return apiFetch<{ id: string; status: string }>(
    `/ddl-providers/${encodeURIComponent(id)}/downloads`,
    { method: "POST", body: JSON.stringify(body) }
  );
}

/** Cadence of the global background DDL search (`0` = disabled). */
export type DdlSearchSettingsDto = {
  search_interval_minutes: number;
};

/** Read the background DDL search cadence. */
export async function fetchDdlSearchSettings(): Promise<DdlSearchSettingsDto> {
  return apiFetch<DdlSearchSettingsDto>("/ddl-providers/search-settings", {
    next: { revalidate: 60 },
  });
}

/** Update the background DDL search cadence (`0` disables it). */
export async function updateDdlSearchSettings(
  searchIntervalMinutes: number
): Promise<DdlSearchSettingsDto> {
  return apiFetch<DdlSearchSettingsDto>("/ddl-providers/search-settings", {
    method: "POST",
    body: JSON.stringify({ search_interval_minutes: searchIntervalMinutes }),
  });
}

/** Queue a background search over every enabled provider. */
export async function startDdlSearchAll(): Promise<{ id: string; status: string }> {
  return apiFetch<{ id: string; status: string }>("/ddl-providers/search-all", {
    method: "POST",
  });
}

/** Download history across every DDL provider. */
export async function fetchDdlProviderDownloads(): Promise<DdlProviderDownloadDto[]> {
  return apiFetch<DdlProviderDownloadDto[]>("/ddl-provider-downloads");
}

/** Dismiss (soft) or delete (`hard`) a download row. */
export async function dismissDdlProviderDownload(id: string, hard = false): Promise<void> {
  return apiFetch<void>(`/ddl-provider-downloads/${encodeURIComponent(id)}${hard ? "?hard=true" : ""}`, {
    method: "DELETE",
  });
}
