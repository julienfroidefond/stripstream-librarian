import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { DownloadRow } from "@/app/(app)/downloads/DownloadsPage";
import type { TorrentDownloadDto } from "@/lib/api";

const fetchMock = vi.fn();

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

function ok(payload: unknown = null) {
  return { ok: true, status: 200, json: async () => payload, text: async () => "" };
}

function torrentDownload(overrides: Partial<TorrentDownloadDto> = {}): TorrentDownloadDto {
  return {
    id: "dl-1",
    library_id: "lib-1",
    series_id: "series-1",
    series_name: "Astérix",
    expected_volumes: [41],
    client: "qbittorrent",
    qb_hash: "hash-1",
    name: null,
    content_path: null,
    status: "error",
    imported_files: null,
    error_message: null,
    progress: 0,
    download_speed: 0,
    eta: 0,
    seeders: null,
    created_at: "2026-01-01T00:00:00Z",
    updated_at: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

function renderRow(dl: TorrentDownloadDto, onRetried = vi.fn()) {
  render(<DownloadRow dl={dl} onDeleted={vi.fn()} onRetried={onRetried} />);
  return onRetried;
}

function clickRetry() {
  fireEvent.click(screen.getByTitle("downloads.retry"));
}

describe("DownloadRow — retry routing", () => {
  it("restarts the magnet for a failed AllDebrid download without local files", async () => {
    fetchMock.mockResolvedValue(ok({ ok: true }));
    const onRetried = renderRow(torrentDownload({ client: "alldebrid", status: "error", content_path: null }));

    clickRetry();

    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith("/api/alldebrid/restart/dl-1", { method: "POST" }));
    await waitFor(() => expect(onRetried).toHaveBeenCalled());
  });

  it("retries the import for an AllDebrid download that already has local files", async () => {
    fetchMock.mockResolvedValue(ok({ ok: true }));
    renderRow(torrentDownload({ client: "alldebrid", status: "error", content_path: "/downloads/sl-1/vol-41.cbz" }));

    clickRetry();

    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith("/api/torrent-downloads/dl-1/retry", { method: "POST" }));
  });

  it("retries the import for a qBittorrent download", async () => {
    fetchMock.mockResolvedValue(ok({ ok: true }));
    renderRow(torrentDownload({ client: "qbittorrent", status: "partial", content_path: null }));

    clickRetry();

    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith("/api/torrent-downloads/dl-1/retry", { method: "POST" }));
  });

  it("does not report success when the restart fails", async () => {
    fetchMock.mockResolvedValue({ ok: false, status: 400, json: async () => ({ error: "magnet expired" }) });
    const onRetried = renderRow(torrentDownload({ client: "alldebrid", status: "error", content_path: null }));

    clickRetry();

    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith("/api/alldebrid/restart/dl-1", { method: "POST" }));
    expect(onRetried).not.toHaveBeenCalled();
  });
});

describe("DownloadRow — torrent details", () => {
  it("shows the torrent name reported by the backend", () => {
    renderRow(torrentDownload({ client: "alldebrid", name: "Aposimz T04.cbr", status: "downloading" }));

    expect(screen.getByText("Aposimz T04.cbr")).toBeInTheDocument();
  });

  it("shows the seeder count for an AllDebrid download", () => {
    renderRow(torrentDownload({ client: "alldebrid", status: "downloading", seeders: 0 }));

    expect(screen.getByText('downloads.seeders:{"count":"0","plural":"s"}')).toBeInTheDocument();
  });

  it("hides the seeder count when the backend does not report it", () => {
    renderRow(torrentDownload({ client: "qbittorrent", status: "downloading", seeders: null }));

    expect(screen.queryByText(/downloads\.seeders/)).not.toBeInTheDocument();
  });

  it("keeps the progress bar at 100% while the download is importing", () => {
    renderRow(torrentDownload({ client: "alldebrid", status: "importing", progress: 1 }));

    expect(screen.getByText("100%")).toBeInTheDocument();
  });

  it("hides the bar for an import retried from a partially-downloaded state", () => {
    renderRow(torrentDownload({ client: "alldebrid", status: "importing", progress: 0.6 }));

    expect(screen.queryByText("60%")).not.toBeInTheDocument();
  });

  it("hides the bar while the download is still transferring", () => {
    renderRow(torrentDownload({ client: "alldebrid", status: "downloading", progress: 0.6 }));

    expect(screen.getByText("60%")).toBeInTheDocument();
  });
});
