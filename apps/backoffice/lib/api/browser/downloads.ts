/** Browser-side acquisition downloads, availability and release blacklist. */

import type {
  DownloadClient,
  LatestFoundPerLibraryDto,
  QBittorrentAddResponse,
  TorrentDownloadDto,
} from "../downloads";
import { browserFetch, browserFetchVoid, withQuery } from "./transport";

export interface TorrentAddInput {
  url: string;
  library_id?: string;
  series_name?: string;
  expected_volumes?: number[];
  replace_existing?: boolean;
}

export interface ReleaseBlacklistItem {
  id: string;
  title: string;
  indexer: string | null;
  series_name: string | null;
}

const ADD_ENDPOINTS: Record<DownloadClient, string> = {
  qbittorrent: "/api/qbittorrent/add",
  alldebrid: "/api/alldebrid/add",
};

export const downloadsApi = {
  torrents: {
    list(): Promise<TorrentDownloadDto[]> {
      return browserFetch<TorrentDownloadDto[]>("/api/torrent-downloads");
    },

    remove(id: string): Promise<void> {
      return browserFetchVoid(`/api/torrent-downloads/${encodeURIComponent(id)}`, {
        method: "DELETE",
      });
    },

    retry(id: string): Promise<void> {
      return browserFetchVoid(`/api/torrent-downloads/${encodeURIComponent(id)}/retry`, {
        method: "POST",
      });
    },
  },

  alldebrid: {
    /** Restart a failed AllDebrid magnet that never produced local files. */
    restart(torrentDownloadId: string): Promise<void> {
      return browserFetchVoid(
        `/api/alldebrid/restart/${encodeURIComponent(torrentDownloadId)}`,
        { method: "POST" }
      );
    },
  },

  detection: {
    latest(): Promise<LatestFoundPerLibraryDto[]> {
      return browserFetch<LatestFoundPerLibraryDto[]>(
        "/api/download-detection/latest-found"
      );
    },
  },

  available: {
    /** Remove one release from a series' available downloads. */
    /**
     * Remove release `releaseIndex` of an available entry. `title` is the title
     * the caller expects at that index: the API answers `409` without changing
     * anything when the list moved since it was read.
     */
    removeRelease(
      seriesId: string,
      releaseIndex: number,
      blacklist = false,
      title?: string
    ): Promise<void> {
      return browserFetchVoid(
        withQuery(`/api/available-downloads/${encodeURIComponent(seriesId)}`, {
          release: releaseIndex,
          title,
          blacklist: blacklist ? "true" : undefined,
        }),
        { method: "DELETE" }
      );
    },

    removeEntry(entryId: string): Promise<void> {
      return browserFetchVoid(
        `/api/available-downloads/${encodeURIComponent(entryId)}`,
        { method: "DELETE" }
      );
    },
  },

  blacklist: {
    list(): Promise<ReleaseBlacklistItem[]> {
      return browserFetch<ReleaseBlacklistItem[]>("/api/release-blacklist");
    },

    remove(id: string): Promise<void> {
      return browserFetchVoid(`/api/release-blacklist/${encodeURIComponent(id)}`, {
        method: "DELETE",
      });
    },
  },

  /** Snatch a release through the active download client. */
  add(client: DownloadClient, input: TorrentAddInput): Promise<QBittorrentAddResponse & { error?: string }> {
    return browserFetch<QBittorrentAddResponse & { error?: string }>(
      ADD_ENDPOINTS[client],
      { method: "POST", body: input }
    );
  },
};
