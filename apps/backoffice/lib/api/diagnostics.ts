/**
 * Diagnostic de santé agrégé exposé par l'API (`GET /diagnostics`) et sonde
 * à la demande (`GET /diagnostics/probe`).
 *
 * Aucune donnée sensible n'y figure : versions, tailles, compteurs et
 * répertoires de cache uniquement.
 */

import { apiFetch } from "./client";

export type DiagnosticsService = {
  service: string;
  version: string;
  online: boolean;
  seconds_since_seen: number;
  uptime_seconds: number;
};

export type DiagnosticsDatabase = {
  reachable: boolean;
  latency_ms: number;
  size_bytes: number;
  size_pretty: string;
  migrations_applied: number;
  latest_migration: string | null;
  active_connections: number;
  max_connections: number;
};

export type DiagnosticsRuntime = {
  rss_bytes: number | null;
  rss_pretty: string | null;
  open_fds: number | null;
  renders_in_flight: number;
  render_limit: number;
};

export type DiagnosticsDisk = {
  /** `libraries`, `thumbnails` or `image_cache`. */
  id: string;
  path: string;
  available: boolean;
  total_bytes: number;
  free_bytes: number;
  used_percent: number;
};

export type DiagnosticsCache = {
  memory_size_mb: number;
  memory_page_count: number;
  memory_max_size_mb: number;
  disk_size_mb: number;
  disk_file_count: number;
  directory: string;
  hits: number;
  misses: number;
};

export type DiagnosticsThumbnails = {
  total_size_bytes: number;
  total_size_pretty: string;
  file_count: number;
  directory: string;
  missing: number;
  pending_analysis: number;
};

export type DiagnosticsFailedJob = {
  id: string;
  job_type: string;
  library_name: string | null;
  finished_at: string | null;
  error: string | null;
};

export type DiagnosticsJobType = {
  job_type: string;
  pending: number;
  running: number;
  failed_last_24h: number;
  last_finished_at: string | null;
};

export type DiagnosticsJobs = {
  total: number;
  pending: number;
  running: number;
  failed_last_24h: number;
  /** Pending for more than 30 minutes: they block the scheduler. */
  stale_pending: number;
  oldest_pending_seconds: number | null;
  by_type: DiagnosticsJobType[];
  recent_failures: DiagnosticsFailedJob[];
};

export type DiagnosticsIntegrity = {
  books_without_files: number;
  books_zero_pages: number;
  series_without_metadata: number;
  duplicate_volume_groups: number;
};

export type DiagnosticsLibrary = {
  id: string;
  name: string;
  enabled: boolean;
  book_count: number;
  pending_analysis: number;
  missing_thumbnails: number;
  size_bytes: number;
  size_pretty: string;
  last_scan_at: string | null;
  next_scan_at: string | null;
  last_metadata_refresh_at: string | null;
  next_metadata_refresh_at: string | null;
  last_download_detection_at: string | null;
  next_download_detection_at: string | null;
  last_reading_status_push_at: string | null;
  next_reading_status_push_at: string | null;
};

export type DiagnosticsResponse = {
  version: string;
  uptime_seconds: number;
  services: DiagnosticsService[];
  database: DiagnosticsDatabase;
  runtime: DiagnosticsRuntime;
  disks: DiagnosticsDisk[];
  cache: DiagnosticsCache;
  thumbnails: DiagnosticsThumbnails;
  jobs: DiagnosticsJobs;
  integrity: DiagnosticsIntegrity;
  libraries: DiagnosticsLibrary[];
};

export type DiagnosticsIntegration = {
  /** `qbittorrent`, `prowlarr`, `telegram` or `ddl_<provider>`. */
  id: string;
  configured: boolean;
  reachable: boolean;
  latency_ms: number;
  detail: string | null;
};

export type DiagnosticsMissingFiles = {
  checked: number;
  missing: number;
  samples: string[];
};

export type DiagnosticsProbe = {
  sampled_at: string;
  integrations: DiagnosticsIntegration[];
  missing_files: DiagnosticsMissingFiles;
};

export async function fetchDiagnostics(): Promise<DiagnosticsResponse> {
  return apiFetch<DiagnosticsResponse>("/diagnostics", { cache: "no-store" });
}

export async function fetchDiagnosticsProbe(): Promise<DiagnosticsProbe> {
  return apiFetch<DiagnosticsProbe>("/diagnostics/probe", { cache: "no-store" });
}
