import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { DownloadsIndicator } from "@/app/components/DownloadsIndicator";

class FakeEventSource {
  static instances: FakeEventSource[] = [];

  url: string;
  closed = false;
  onmessage: ((event: { data: string }) => void) | null = null;
  onerror: (() => void) | null = null;

  constructor(url: string) {
    this.url = url;
    FakeEventSource.instances.push(this);
  }

  close() {
    this.closed = true;
  }

  emit(data: unknown) {
    this.onmessage?.({ data: JSON.stringify(data) });
  }
}

const STREAM_URL = "/api/downloads/stream";

function sourceFor(url: string): FakeEventSource {
  const source = FakeEventSource.instances.find((instance) => instance.url === url);
  if (!source) throw new Error(`no EventSource opened for ${url}`);
  return source;
}

/** Emits a `/api/downloads/stream` frame with the sources not listed left empty. */
function emit(payload: {
  torrent?: unknown[];
  telegram?: unknown[];
  ddl?: unknown[];
}) {
  act(() => sourceFor(STREAM_URL).emit({ torrent: [], telegram: [], ddl: [], ...payload }));
}

function ddlDownload(overrides: Record<string, unknown> = {}) {
  return {
    id: "d1",
    provider: "fourtoutici",
    filename: "Astérix - T1.cbz",
    file_size: 100,
    bytes_downloaded: 50,
    series_name: "Astérix",
    series_id: "s1",
    library_id: "l1",
    volume_number: 1,
    status: "downloading",
    error_message: null,
    ...overrides,
  };
}

/** The badge title carries the active download count via the `downloadCount` key. */
function badgeCount(): number | null {
  const match = screen.getByTitle(/downloadsIndicator\.downloadCount/);
  const title = match.getAttribute("title") ?? "";
  const parsed = title.match(/"count":(\d+)/);
  return parsed ? Number(parsed[1]) : null;
}

beforeEach(() => {
  FakeEventSource.instances = [];
  vi.stubGlobal("EventSource", FakeEventSource);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("DownloadsIndicator", () => {
  it("shows only the shortcut when nothing is downloading", () => {
    render(<DownloadsIndicator />);

    expect(screen.getByTitle("downloadsIndicator.viewAll")).toBeInTheDocument();
  });

  it("watches every source over a single connection", () => {
    render(<DownloadsIndicator />);

    // One long-lived connection per source would saturate the browser's
    // per-origin cap and stall client-side navigation.
    expect(FakeEventSource.instances).toHaveLength(1);
    expect(FakeEventSource.instances[0].url).toBe(STREAM_URL);
  });

  it("counts active DDL downloads in the badge", () => {
    render(<DownloadsIndicator />);

    emit({
      ddl: [
        ddlDownload({ id: "d1", status: "downloading" }),
        ddlDownload({ id: "d2", status: "imported" }),
        ddlDownload({ id: "d3", status: "failed" }),
      ],
    });

    expect(badgeCount()).toBe(1);
  });

  it("counts queued DDL downloads too", () => {
    render(<DownloadsIndicator />);

    emit({ ddl: [ddlDownload({ id: "d1", status: "queued", bytes_downloaded: 0 })] });

    expect(badgeCount()).toBe(1);
  });

  it("shows a queued DDL download as queued rather than downloading", () => {
    render(<DownloadsIndicator />);

    emit({ ddl: [ddlDownload({ id: "d1", status: "queued", bytes_downloaded: 0 })] });

    fireEvent.click(screen.getByTitle(/downloadsIndicator\.downloadCount/));

    expect(screen.getByText("downloads.status.queued")).toBeInTheDocument();
    expect(screen.queryByText("downloads.status.downloading")).not.toBeInTheDocument();
  });

  it("aggregates torrent, telegram and DDL downloads", () => {
    render(<DownloadsIndicator />);

    emit({
      torrent: [
        {
          id: "t1",
          source: "prowlarr",
          library_id: "l1",
          series_name: "Berserk",
          expected_volumes: [1],
          status: "downloading",
          progress: 0.5,
          download_speed: 0,
          eta: 0,
          error_message: null,
        },
      ],
      telegram: [
        {
          id: "tg1",
          series_name: "Berserk",
          series_id: null,
          library_id: "l1",
          filename: "Berserk - T2.cbz",
          file_size: 100,
          bytes_downloaded: 10,
          volume_number: 2,
          status: "downloading",
          error_message: null,
        },
      ],
      ddl: [ddlDownload()],
    });

    expect(badgeCount()).toBe(3);
  });

  it("drops DDL downloads once they are imported", () => {
    render(<DownloadsIndicator />);

    emit({ ddl: [ddlDownload({ status: "downloading" })] });
    expect(badgeCount()).toBe(1);

    emit({ ddl: [ddlDownload({ status: "imported" })] });
    expect(screen.getByTitle("downloadsIndicator.viewAll")).toBeInTheDocument();
  });
});
