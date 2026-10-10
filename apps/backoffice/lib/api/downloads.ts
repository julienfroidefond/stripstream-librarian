/**
 * Téléchargements : Prowlarr, qBittorrent, détection de releases.
 */

import { apiFetch } from "./client";

export async function startDownloadDetection(libraryId?: string) {
  return apiFetch<{ id: string | null; status: string }>("/download-detection/start", {
    method: "POST",
    body: JSON.stringify(libraryId ? { library_id: libraryId } : {}),
  });
}

export async function startRssPoll(libraryId?: string) {
  return apiFetch<{ id: string | null; status: string }>("/prowlarr-rss/start", {
    method: "POST",
    body: JSON.stringify(libraryId ? { library_id: libraryId } : {}),
  });
}

export type AvailableReleaseDto = {
  title: string;
  size: number;
  download_url: string | null;
  indexer: string | null;
  seeders: number | null;
  matched_missing_volumes: number[];
  all_volumes: number[];
  has_failed?: boolean;
  detected_at?: string;
  match_confidence?: "high" | "review";
  match_reasons?: string[];
};

export type DownloadDetectionReportDto = {
  job_id: string;
  status: string;
  total_series: number;
  found: number;
  new_releases?: number;
  not_found: number;
  no_missing: number;
  no_metadata: number;
  errors: number;
};

export type DownloadDetectionResultDto = {
  id: string;
  series_id?: string;
  series_name: string;
  status: "found" | "not_found" | "no_missing" | "no_metadata" | "error";
  missing_count: number;
  available_releases: AvailableReleaseDto[] | null;
  error_message: string | null;
};

export type AvailableDownloadDto = {
  id: string;
  series_id: string;
  series_name: string;
  missing_count: number;
  available_releases: AvailableReleaseDto[] | null;
  updated_at: string;
  failed_download_count: number;
};

export type LatestFoundPerLibraryDto = {
  library_id: string;
  library_name: string;
  results: AvailableDownloadDto[];
};

export async function getDownloadDetectionReport(jobId: string) {
  return apiFetch<DownloadDetectionReportDto>(`/download-detection/${jobId}/report`);
}

export async function getDownloadDetectionResults(jobId: string, status?: string) {
  const url = status
    ? `/download-detection/${jobId}/results?status=${encodeURIComponent(status)}`
    : `/download-detection/${jobId}/results`;
  return apiFetch<DownloadDetectionResultDto[]>(url);
}

export type ProwlarrCategory = {
  id: number;
  name: string | null;
};

export type ProwlarrRelease = {
  guid: string;
  title: string;
  size: number;
  downloadUrl: string | null;
  indexer: string | null;
  seeders: number | null;
  leechers: number | null;
  publishDate: string | null;
  protocol: string | null;
  infoUrl: string | null;
  categories: ProwlarrCategory[] | null;
  matchedMissingVolumes: number[] | null;
  allVolumes?: number[];
  matchConfidence?: "high" | "review";
  matchReasons?: string[];
};

export type ProwlarrSearchResponse = {
  results: ProwlarrRelease[];
  query: string;
};

export type ProwlarrTestResponse = {
  success: boolean;
  message: string;
  indexer_count: number | null;
};

export type QBittorrentAddResponse = {
  success: boolean;
  message: string;
  torrent_download_id?: string | null;
};

export type TorrentDownloadDto = {
  id: string;
  library_id: string;
  series_id?: string;
  series_name: string;
  expected_volumes: number[];
  qb_hash: string | null;
  content_path: string | null;
  status: "downloading" | "completed" | "importing" | "imported" | "partial" | "no_files_imported" | "error";
  imported_files: Array<{ volume: number; source: string; destination: string; already_existed?: boolean }> | null;
  error_message: string | null;
  progress: number;
  download_speed: number;
  eta: number;
  created_at: string;
  updated_at: string;
};

export async function fetchTorrentDownloads(): Promise<TorrentDownloadDto[]> {
  return apiFetch<TorrentDownloadDto[]>("/torrent-downloads");
}

export type QBittorrentTestResponse = {
  success: boolean;
  message: string;
  version: string | null;
};
