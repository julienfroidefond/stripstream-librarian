/**
 * Contract of `/api/downloads/stream`, shared by the route handler and the
 * client components consuming it.
 */

import type { DdlProviderDownloadDto } from "@/lib/api/ddl-providers";
import type { TelegramDownloadItemDto } from "@/lib/api/telegram";
import type { TorrentDownloadDto } from "@/lib/api/downloads";

export const DOWNLOADS_STREAM_URL = "/api/downloads/stream";

export type DownloadsStreamPayload = {
  torrent: TorrentDownloadDto[];
  telegram: TelegramDownloadItemDto[];
  ddl: DdlProviderDownloadDto[];
};

type StatusItem = { status: string };

/**
 * Whether any source still has a moving item.
 *
 * Drives the SSE polling cadence: fast while downloading, slow when idle.
 */
export function hasActiveDownloads({
  torrent = [],
  telegram = [],
  ddl = [],
}: {
  torrent?: StatusItem[];
  telegram?: StatusItem[];
  ddl?: StatusItem[];
}): boolean {
  return (
    torrent.some(item => item.status === "downloading" || item.status === "completed" || item.status === "importing") ||
    telegram.some(item => item.status === "downloading") ||
    ddl.some(item => item.status === "queued" || item.status === "downloading")
  );
}
