import { describe, expect, it } from "vitest";

import type { DdlProviderDownloadDto } from "@/lib/api/ddl-providers";
import type { TelegramDownloadItemDto } from "@/lib/api/telegram";
import type { TorrentDownloadDto } from "@/lib/api/downloads";
import {
  indicatorDownloads,
  matchesDownloadFilter,
  mergeDownloads,
  normalizeDdlDownload,
  normalizeTelegramDownload,
  normalizeTorrentDownload,
} from "@/lib/downloads/model";

function torrent(overrides: Partial<TorrentDownloadDto> = {}): TorrentDownloadDto {
  return {
    id: "t1",
    library_id: "lib-1",
    series_id: "series-1",
    series_name: "Berserk",
    expected_volumes: [1],
    client: "qbittorrent",
    qb_hash: "hash",
    name: "Berserk - T1",
    content_path: null,
    status: "downloading",
    imported_files: null,
    error_message: null,
    progress: 0.5,
    file_size: 100,
    download_speed: 1024,
    eta: 60,
    seeders: null,
    created_at: "2026-01-01T10:00:00Z",
    updated_at: "2026-01-01T10:05:00Z",
    ...overrides,
  };
}

function telegram(overrides: Partial<TelegramDownloadItemDto> = {}): TelegramDownloadItemDto {
  return {
    id: "tg1",
    series_name: "Berserk",
    series_id: "series-1",
    library_id: "lib-1",
    library_name: "BD",
    channel_username: "channel",
    filename: "Berserk - T2.cbz",
    file_size: 100,
    bytes_downloaded: 25,
    download_speed: 2048,
    eta: 30,
    volume_number: 2,
    status: "downloading",
    error_message: null,
    created_at: "2026-01-01T09:00:00Z",
    updated_at: "2026-01-01T09:05:00Z",
    ...overrides,
  };
}

function ddl(overrides: Partial<DdlProviderDownloadDto> = {}): DdlProviderDownloadDto {
  return {
    id: "ddl1",
    provider: "fourtoutici",
    external_id: "ext-1",
    filename: "Berserk - T3.pdf",
    file_size: 100,
    bytes_downloaded: 50,
    download_speed: 4096,
    eta: 15,
    series_name: "Berserk",
    series_id: "series-1",
    volume_number: 3,
    library_id: "lib-1",
    library_name: "BD",
    status: "downloading",
    error_message: null,
    created_at: "2026-01-01T08:00:00Z",
    updated_at: "2026-01-01T08:05:00Z",
    ...overrides,
  };
}

describe("normalizeTorrentDownload", () => {
  it("namespaces the id and keeps torrent fields", () => {
    const item = normalizeTorrentDownload(torrent());

    expect(item).toMatchObject({
      kind: "torrent",
      source: "prowlarr",
      key: "torrent-t1",
      status: "downloading",
      progress: 0.5,
      download_speed: 1024,
      series_name: "Berserk",
    });
    expect(item.data.id).toBe("t1");
  });
});

describe("normalizeTelegramDownload", () => {
  it("maps failed to error and derives progress from the transfer size", () => {
    const item = normalizeTelegramDownload(
      telegram({ status: "failed", bytes_downloaded: 50 }),
    );

    expect(item.status).toBe("error");
    expect(item.progress).toBe(0.5);
    expect(item.key).toBe("telegram-tg1");
    expect(item.expected_volumes).toEqual([2]);
  });

  it("clamps progress and falls back to the filename for unnamed series", () => {
    const item = normalizeTelegramDownload(
      telegram({ bytes_downloaded: 150, series_name: null, library_id: null }),
    );

    expect(item.progress).toBe(1);
    expect(item.series_name).toBe("Berserk - T2.cbz");
    expect(item.library_id).toBe("");
  });

  it("reports zero progress when the file size is unknown", () => {
    const item = normalizeTelegramDownload(telegram({ file_size: null }));

    expect(item.progress).toBe(0);
  });
});

describe("normalizeDdlDownload", () => {
  it("keeps the provider and maps failed to error", () => {
    const item = normalizeDdlDownload(ddl({ status: "failed", bytes_downloaded: 0 }));

    expect(item.status).toBe("error");
    expect(item.provider).toBe("fourtoutici");
    expect(item.progress).toBe(0);
    expect(item.key).toBe("ddl-ddl1");
  });

  it("keeps a queued provider download queued", () => {
    const item = normalizeDdlDownload(ddl({ status: "queued", bytes_downloaded: 0 }));

    expect(item.status).toBe("queued");
  });
});

describe("mergeDownloads", () => {
  it("puts active downloads first, oldest first, then inactive by newest update", () => {
    const merged = mergeDownloads(
      [
        torrent({
          id: "active-late",
          status: "downloading",
          created_at: "2026-01-02T00:00:00Z",
        }),
        torrent({
          id: "done-old",
          status: "imported",
          updated_at: "2026-01-03T00:00:00Z",
        }),
      ],
      [
        telegram({
          id: "active-early",
          status: "downloading",
          created_at: "2026-01-01T00:00:00Z",
        }),
        telegram({
          id: "done-new",
          status: "imported",
          updated_at: "2026-01-04T00:00:00Z",
        }),
      ],
      [ddl({ id: "queued", status: "queued", created_at: "2026-01-03T00:00:00Z" })],
    );

    expect(merged.map((item) => item.key)).toEqual([
      "telegram-active-early",
      "torrent-active-late",
      "ddl-queued",
      "telegram-done-new",
      "torrent-done-old",
    ]);
  });
});

describe("matchesDownloadFilter", () => {
  it("treats torrents through import as active", () => {
    for (const status of ["downloading", "completed", "importing"] as const) {
      const item = normalizeTorrentDownload(torrent({ status }));
      expect(matchesDownloadFilter(item, "active")).toBe(true);
    }
    expect(
      matchesDownloadFilter(normalizeTorrentDownload(torrent({ status: "imported" })), "active"),
    ).toBe(false);
  });

  it("treats only queued and downloading secondary downloads as active", () => {
    const queued = normalizeTelegramDownload(telegram({ status: "queued" }));
    const downloading = normalizeDdlDownload(ddl({ status: "downloading" }));
    const imported = normalizeDdlDownload(ddl({ status: "imported" }));

    expect(matchesDownloadFilter(queued, "active")).toBe(true);
    expect(matchesDownloadFilter(downloading, "active")).toBe(true);
    expect(matchesDownloadFilter(imported, "active")).toBe(false);
  });

  it("matches failed secondary downloads on the error filter", () => {
    const failed = normalizeTelegramDownload(telegram({ status: "failed" }));

    expect(matchesDownloadFilter(failed, "error")).toBe(true);
    expect(matchesDownloadFilter(failed, "imported")).toBe(false);
  });

  it("matches every item on the all filter", () => {
    expect(matchesDownloadFilter(normalizeDdlDownload(ddl()), "all")).toBe(true);
  });
});

describe("indicatorDownloads", () => {
  it("keeps the narrow indicator semantics for every source", () => {
    const items = indicatorDownloads(
      [
        torrent({ id: "t-downloading", status: "downloading" }),
        torrent({ id: "t-importing", status: "importing" }),
        torrent({ id: "t-imported", status: "imported" }),
      ],
      [
        telegram({ id: "tg-queued", status: "queued" }),
        telegram({ id: "tg-downloading", status: "downloading" }),
        telegram({ id: "tg-imported", status: "imported" }),
      ],
      [
        ddl({ id: "ddl-queued", status: "queued" }),
        ddl({ id: "ddl-downloading", status: "downloading" }),
        ddl({ id: "ddl-imported", status: "imported" }),
      ],
    );

    expect(items.map((item) => item.key)).toEqual([
      "torrent-t-downloading",
      "torrent-t-importing",
      "telegram-tg-downloading",
      "ddl-ddl-queued",
      "ddl-ddl-downloading",
    ]);
  });
});
