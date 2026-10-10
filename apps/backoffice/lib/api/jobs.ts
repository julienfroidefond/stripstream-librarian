/**
 * Jobs : liste, déclenchement, rapports de batch / refresh / statut de lecture.
 */

import { apiFetch } from "./client";

export type IndexJobDto = {
  id: string;
  library_id: string | null;
  library_name: string | null;
  book_id: string | null;
  type: string;
  status: string;
  started_at: string | null;
  finished_at: string | null;
  error_opt: string | null;
  created_at: string;
  stats_json: {
    scanned_files: number;
    indexed_files: number;
    removed_files: number;
    errors: number;
    warnings: number;
    refreshed?: number;
    found?: number;
    new_releases?: number;
    linked?: number;
    pushed?: number;
    new_books?: number;
    series_searched?: number;
    sources_scanned?: number;
    all_series?: Array<{ series_name: string; book_count: number; extracted_names?: string[] }>;
    matched_series?: Array<unknown>;
  } | null;
  progress_percent: number | null;
  processed_files: number | null;
  total_files: number | null;
  current_file: string | null;
};

export async function listJobs() {
  return apiFetch<IndexJobDto[]>("/index/status");
}

export async function rebuildIndex(libraryId?: string, full?: boolean, rescan?: boolean) {
  const body: { library_id?: string; full?: boolean; rescan?: boolean } = {};
  if (libraryId) body.library_id = libraryId;
  if (full) body.full = true;
  if (rescan) body.rescan = true;
  return apiFetch<IndexJobDto>("/index/rebuild", {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export async function rebuildThumbnails(libraryId?: string) {
  const body: { library_id?: string } = {};
  if (libraryId) body.library_id = libraryId;
  return apiFetch<IndexJobDto>("/index/thumbnails/rebuild", {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export async function regenerateThumbnails(libraryId?: string) {
  const body: { library_id?: string } = {};
  if (libraryId) body.library_id = libraryId;
  return apiFetch<IndexJobDto>("/index/thumbnails/regenerate", {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export async function cancelJob(id: string) {
  return apiFetch<IndexJobDto>(`/index/cancel/${id}`, { method: "POST" });
}

export type MetadataBatchReportDto = {
  job_id: string;
  status: string;
  total_series: number;
  processed: number;
  auto_matched: number;
  no_results: number;
  too_many_results: number;
  low_confidence: number;
  already_linked: number;
  errors: number;
};

export type MetadataBatchResultDto = {
  id: string;
  series_id?: string;
  series_name: string;
  status: string;
  provider_used: string | null;
  fallback_used: boolean;
  candidates_count: number;
  best_confidence: number | null;
  best_candidate_json: Record<string, unknown> | null;
  link_id: string | null;
  error_message: string | null;
};

export async function startMetadataBatch(libraryId?: string, forceRematch = false) {
  const payload: Record<string, unknown> = {};
  if (libraryId) payload.library_id = libraryId;
  if (forceRematch) payload.force_rematch = true;
  return apiFetch<{ id: string | null; status: string }>("/metadata/batch", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}

export async function startMetadataRefresh(libraryId?: string) {
  return apiFetch<{ id: string | null; status: string }>("/metadata/refresh", {
    method: "POST",
    body: JSON.stringify(libraryId ? { library_id: libraryId } : {}),
  });
}

export async function startMetadataRefreshAll(libraryId?: string) {
  return apiFetch<{ id: string | null; status: string }>("/metadata/refresh-all", {
    method: "POST",
    body: JSON.stringify(libraryId ? { library_id: libraryId } : {}),
  });
}

export async function startReadingStatusMatch(libraryId?: string) {
  return apiFetch<{ id: string | null; status: string }>("/reading-status/match", {
    method: "POST",
    body: JSON.stringify(libraryId ? { library_id: libraryId } : {}),
  });
}

export type ReadingStatusMatchReportDto = {
  job_id: string;
  status: string;
  total_series: number;
  linked: number;
  already_linked: number;
  no_results: number;
  ambiguous: number;
  errors: number;
};

export type ReadingStatusMatchResultDto = {
  id: string;
  series_id?: string;
  series_name: string;
  status: "linked" | "already_linked" | "no_results" | "ambiguous" | "error";
  anilist_id: number | null;
  anilist_title: string | null;
  anilist_url: string | null;
  error_message: string | null;
};

export async function getReadingStatusMatchReport(jobId: string) {
  return apiFetch<ReadingStatusMatchReportDto>(`/reading-status/match/${jobId}/report`);
}

export async function getReadingStatusMatchResults(jobId: string) {
  return apiFetch<ReadingStatusMatchResultDto[]>(`/reading-status/match/${jobId}/results`);
}

export async function startReadingStatusPush(libraryId?: string) {
  return apiFetch<{ id: string | null; status: string }>("/reading-status/push", {
    method: "POST",
    body: JSON.stringify(libraryId ? { library_id: libraryId } : {}),
  });
}

export async function startReadingStatusPull() {
  return apiFetch<{ id: string | null; status: string }>("/ratings/pull", {
    method: "POST",
  });
}

export type ReadingStatusPushReportDto = {
  job_id: string;
  status: string;
  total_series: number;
  pushed: number;
  skipped: number;
  no_books: number;
  errors: number;
};

export type ReadingStatusPushResultDto = {
  id: string;
  series_id?: string;
  series_name: string;
  status: "pushed" | "skipped" | "no_books" | "error";
  anilist_id: number | null;
  anilist_title: string | null;
  anilist_url: string | null;
  anilist_status: string | null;
  progress_volumes: number | null;
  error_message: string | null;
};

export async function getReadingStatusPushReport(jobId: string) {
  return apiFetch<ReadingStatusPushReportDto>(`/reading-status/push/${jobId}/report`);
}

export async function getReadingStatusPushResults(jobId: string) {
  return apiFetch<ReadingStatusPushResultDto[]>(`/reading-status/push/${jobId}/results`);
}

export type RefreshFieldDiff = {
  field: string;
  old?: unknown;
  new?: unknown;
};

export type RefreshBookDiff = {
  book_id: string;
  title: string;
  volume: number | null;
  changes: RefreshFieldDiff[];
};

export type RefreshSeriesResult = {
  series_id?: string;
  series_name: string;
  provider: string;
  status: string; // "updated" | "unchanged" | "error"
  series_changes: RefreshFieldDiff[];
  book_changes: RefreshBookDiff[];
  error?: string;
};

export type MetadataRefreshReportDto = {
  job_id: string;
  status: string;
  total_links: number;
  refreshed: number;
  unchanged: number;
  errors: number;
  changes: RefreshSeriesResult[];
};

export async function getMetadataRefreshReport(jobId: string) {
  return apiFetch<MetadataRefreshReportDto>(`/metadata/refresh/${jobId}/report`);
}

export async function getMetadataBatchReport(jobId: string) {
  return apiFetch<MetadataBatchReportDto>(`/metadata/batch/${jobId}/report`);
}

export async function getMetadataBatchResults(jobId: string, status?: string) {
  const params = status ? `?status=${status}` : "";
  return apiFetch<MetadataBatchResultDto[]>(`/metadata/batch/${jobId}/results${params}`);
}
