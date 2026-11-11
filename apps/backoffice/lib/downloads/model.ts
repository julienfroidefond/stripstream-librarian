/**
 * Unified read model for the three download sources.
 *
 * Torrents (Prowlarr/qBittorrent/AllDebrid), Telegram and DDL providers expose
 * different DTOs. This module adapts them into one item shape so the downloads
 * page and the navigation indicator share the same merge and filter rules.
 */

import type { DdlProviderDownloadDto, DdlProviderDownloadStatus } from "@/lib/api/ddl-providers";
import type { TelegramDownloadItemDto } from "@/lib/api/telegram";
import type { TorrentDownloadDto } from "@/lib/api/downloads";

export type DownloadKind = "torrent" | "telegram" | "ddl";
export type DownloadSource = "prowlarr" | "telegram" | "ddl";

export type DownloadStatus =
  | "queued"
  | "downloading"
  | "completed"
  | "importing"
  | "imported"
  | "partial"
  | "no_files_imported"
  | "error"
  | "dismissed";

export type DownloadFilter = "all" | "active" | "imported" | "error";

type DownloadItemBase = {
  /** Namespaced id so torrent, Telegram and DDL ids never collide. */
  key: string;
  source: DownloadSource;
  status: DownloadStatus;
  progress: number;
  download_speed: number;
  eta: number;
  error_message: string | null;
  library_id: string;
  series_id?: string;
  series_name: string;
  expected_volumes: number[];
  created_at: string;
  updated_at: string;
  /** DDL providers only: name of the external catalogue. */
  provider?: string;
};

export type TorrentDownloadItem = DownloadItemBase & {
  kind: "torrent";
  data: TorrentDownloadDto;
};

export type TelegramDownloadItem = DownloadItemBase & {
  kind: "telegram";
  data: TelegramDownloadItemDto;
};

export type DdlDownloadItem = DownloadItemBase & {
  kind: "ddl";
  data: DdlProviderDownloadDto;
};

export type DownloadItem =
  | TorrentDownloadItem
  | TelegramDownloadItem
  | DdlDownloadItem;

/** Torrent statuses that mean the transfer or its import is still moving. */
const TORRENT_ACTIVE_STATUSES = new Set<DownloadStatus>([
  "downloading",
  "completed",
  "importing",
]);

/** Any source status that counts as active when merging the list. */
const ACTIVE_STATUSES = new Set<DownloadStatus>([
  "queued",
  "downloading",
  "completed",
  "importing",
]);

function transferProgress(fileSize: number | null, bytesDownloaded: number): number {
  if (!fileSize || fileSize <= 0) return 0;
  return Math.min(1, bytesDownloaded / fileSize);
}

export function normalizeTorrentDownload(data: TorrentDownloadDto): TorrentDownloadItem {
  return {
    kind: "torrent",
    source: "prowlarr",
    key: `torrent-${data.id}`,
    data,
    status: data.status,
    progress: data.progress,
    download_speed: data.download_speed,
    eta: data.eta,
    error_message: data.error_message,
    library_id: data.library_id,
    series_id: data.series_id,
    series_name: data.series_name,
    expected_volumes: data.expected_volumes,
    created_at: data.created_at,
    updated_at: data.updated_at,
  };
}

export function normalizeTelegramDownload(data: TelegramDownloadItemDto): TelegramDownloadItem {
  return {
    kind: "telegram",
    source: "telegram",
    key: `telegram-${data.id}`,
    data,
    status: data.status === "failed" ? "error" : data.status,
    progress: transferProgress(data.file_size, data.bytes_downloaded),
    download_speed: data.download_speed ?? 0,
    eta: data.eta ?? 0,
    error_message: data.error_message,
    library_id: data.library_id ?? "",
    series_id: data.series_id ?? undefined,
    series_name: data.series_name ?? data.filename,
    expected_volumes: data.volume_number != null ? [data.volume_number] : [],
    created_at: data.created_at,
    updated_at: data.updated_at,
  };
}

export function normalizeDdlDownload(data: DdlProviderDownloadDto): DdlDownloadItem {
  return {
    kind: "ddl",
    source: "ddl",
    key: `ddl-${data.id}`,
    data,
    status: data.status === "failed" ? "error" : data.status,
    progress: transferProgress(data.file_size, data.bytes_downloaded),
    download_speed: data.download_speed ?? 0,
    eta: data.eta ?? 0,
    error_message: data.error_message,
    library_id: data.library_id ?? "",
    series_id: data.series_id ?? undefined,
    series_name: data.series_name ?? data.filename,
    expected_volumes: data.volume_number != null ? [data.volume_number] : [],
    created_at: data.created_at,
    updated_at: data.updated_at,
    provider: data.provider,
  };
}

function compareDownloads(a: DownloadItem, b: DownloadItem): number {
  const aActive = ACTIVE_STATUSES.has(a.status);
  const bActive = ACTIVE_STATUSES.has(b.status);
  if (aActive !== bActive) return aActive ? -1 : 1;
  if (aActive) {
    // Active: oldest created_at first — stable, no swapping during progress updates.
    return new Date(a.created_at).getTime() - new Date(b.created_at).getTime();
  }
  // Inactive: most recently updated first — stable since updated_at stops moving after import.
  return new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime();
}

/**
 * Merge every source into one list.
 *
 * Active downloads come first (oldest first) so rows do not jump around while
 * their progress updates; finished downloads follow, most recently updated first.
 */
export function mergeDownloads(
  torrents: TorrentDownloadDto[],
  telegram: TelegramDownloadItemDto[],
  ddl: DdlProviderDownloadDto[],
): DownloadItem[] {
  return [
    ...torrents.map(normalizeTorrentDownload),
    ...telegram.map(normalizeTelegramDownload),
    ...ddl.map(normalizeDdlDownload),
  ].sort(compareDownloads);
}

export function matchesDownloadFilter(item: DownloadItem, filter: DownloadFilter): boolean {
  if (filter === "all") return true;
  if (item.kind === "torrent") {
    if (filter === "active") return TORRENT_ACTIVE_STATUSES.has(item.status);
    return item.status === filter;
  }
  if (filter === "active") return item.status === "queued" || item.status === "downloading";
  return item.status === filter;
}

/**
 * Items shown by the navigation indicator.
 *
 * The indicator is deliberately narrower than the page: Telegram downloads only
 * appear once the transfer actually started, and torrent rows stay visible while
 * importing.
 */
export function indicatorDownloads(
  torrents: TorrentDownloadDto[],
  telegram: TelegramDownloadItemDto[],
  ddl: DdlProviderDownloadDto[],
): DownloadItem[] {
  return [
    ...torrents
      .map(normalizeTorrentDownload)
      .filter(item => TORRENT_ACTIVE_STATUSES.has(item.status)),
    ...telegram
      .map(normalizeTelegramDownload)
      .filter(item => item.status === "downloading"),
    ...ddl
      .map(normalizeDdlDownload)
      .filter(item => item.status === "queued" || item.status === "downloading"),
  ];
}
