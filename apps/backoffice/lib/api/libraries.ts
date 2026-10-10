/**
 * Bibliothèques : CRUD, scan, monitoring, dossiers.
 */

import { apiFetch } from "./client";
import type { IndexJobDto } from "./jobs";

export type LibraryDto = {
  id: string;
  name: string;
  root_path: string;
  enabled: boolean;
  book_count: number;
  monitor_enabled: boolean;
  scan_mode: string;
  next_scan_at: string | null;
  watcher_enabled: boolean;
  metadata_provider: string | null;
  fallback_metadata_provider: string | null;
  metadata_refresh_mode: string;
  next_metadata_refresh_at: string | null;
  series_count: number;
  thumbnail_book_ids: string[];
  reading_status_provider: string | null;
  reading_status_push_mode: string;
  next_reading_status_push_at: string | null;
  download_detection_mode: string;
  next_download_detection_at: string | null;
  tags: string[];
};

export type FolderItem = {
  name: string;
  path: string;
  depth: number;
  has_children: boolean;
};

export async function fetchLibraries() {
  return apiFetch<LibraryDto[]>("/libraries", { next: { revalidate: 30 } });
}

export async function createLibrary(name: string, rootPath: string) {
  return apiFetch<LibraryDto>("/libraries", {
    method: "POST",
    body: JSON.stringify({ name, root_path: rootPath }),
  });
}

export async function deleteLibrary(id: string) {
  return apiFetch<void>(`/libraries/${id}`, { method: "DELETE" });
}

export async function scanLibrary(libraryId: string, full?: boolean) {
  const body: { full?: boolean } = {};
  if (full) body.full = true;
  return apiFetch<IndexJobDto>(`/libraries/${libraryId}/scan`, {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export async function updateLibraryMonitoring(
  libraryId: string,
  monitorEnabled: boolean,
  scanMode: string,
  watcherEnabled?: boolean,
  metadataRefreshMode?: string,
  downloadDetectionMode?: string,
) {
  const body: {
    monitor_enabled: boolean;
    scan_mode: string;
    watcher_enabled?: boolean;
    metadata_refresh_mode?: string;
    download_detection_mode?: string;
  } = {
    monitor_enabled: monitorEnabled,
    scan_mode: scanMode,
  };
  if (watcherEnabled !== undefined) {
    body.watcher_enabled = watcherEnabled;
  }
  if (metadataRefreshMode !== undefined) {
    body.metadata_refresh_mode = metadataRefreshMode;
  }
  if (downloadDetectionMode !== undefined) {
    body.download_detection_mode = downloadDetectionMode;
  }
  return apiFetch<LibraryDto>(`/libraries/${libraryId}/monitoring`, {
    method: "PATCH",
    body: JSON.stringify(body),
  });
}

export async function listFolders(path?: string) {
  const url = path ? `/folders?path=${encodeURIComponent(path)}` : "/folders";
  return apiFetch<FolderItem[]>(url);
}

export async function updateLibraryMetadataProvider(libraryId: string, provider: string | null, fallbackProvider?: string | null) {
  return apiFetch<LibraryDto>(`/libraries/${libraryId}/metadata-provider`, {
    method: "PATCH",
    body: JSON.stringify({ metadata_provider: provider, fallback_metadata_provider: fallbackProvider }),
  });
}
