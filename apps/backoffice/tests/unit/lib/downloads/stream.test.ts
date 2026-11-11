import { describe, expect, it } from "vitest";

import { hasActiveDownloads } from "@/lib/downloads/stream";

describe("hasActiveDownloads", () => {
  it("reports no activity for empty sources", () => {
    expect(hasActiveDownloads({})).toBe(false);
    expect(hasActiveDownloads({ torrent: [], telegram: [], ddl: [] })).toBe(false);
  });

  it("counts torrents through import as active", () => {
    for (const status of ["downloading", "completed", "importing"]) {
      expect(hasActiveDownloads({ torrent: [{ status }] })).toBe(true);
    }
    expect(hasActiveDownloads({ torrent: [{ status: "imported" }] })).toBe(false);
  });

  it("counts only downloading Telegram transfers", () => {
    expect(hasActiveDownloads({ telegram: [{ status: "queued" }] })).toBe(false);
    expect(hasActiveDownloads({ telegram: [{ status: "downloading" }] })).toBe(true);
  });

  it("counts queued and downloading DDL transfers", () => {
    expect(hasActiveDownloads({ ddl: [{ status: "queued" }] })).toBe(true);
    expect(hasActiveDownloads({ ddl: [{ status: "downloading" }] })).toBe(true);
    expect(hasActiveDownloads({ ddl: [{ status: "imported" }] })).toBe(false);
  });

  it("activates the stream when any single source is moving", () => {
    expect(
      hasActiveDownloads({
        torrent: [{ status: "imported" }],
        telegram: [{ status: "imported" }],
        ddl: [{ status: "queued" }],
      }),
    ).toBe(true);
  });
});
