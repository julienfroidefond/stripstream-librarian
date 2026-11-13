import { describe, expect, it, vi } from "vitest";

import type { LibraryDto } from "@/lib/api";
import {
  applyLibraryWrites,
  diffLibrarySettings,
  librarySettingsFromForm,
  librarySettingsOf,
  type LibrarySettings,
  type LibraryWrite,
} from "@/lib/libraries/settings";

function library(overrides: Partial<LibraryDto> = {}): LibraryDto {
  return {
    id: "lib-1",
    name: "Manga",
    root_path: "/libraries/manga",
    enabled: true,
    book_count: 10,
    monitor_enabled: true,
    scan_mode: "daily",
    next_scan_at: null,
    watcher_enabled: false,
    metadata_provider: "anilist",
    fallback_metadata_provider: null,
    metadata_refresh_mode: "weekly",
    next_metadata_refresh_at: null,
    series_count: 2,
    thumbnail_book_ids: [],
    reading_status_provider: null,
    reading_status_push_mode: "manual",
    next_reading_status_push_at: null,
    download_detection_mode: "manual",
    next_download_detection_at: null,
    tags: ["manga"],
    ...overrides,
  };
}

const before: LibrarySettings = librarySettingsOf(library());

function form(fields: Record<string, string>): FormData {
  const data = new FormData();
  for (const [key, value] of Object.entries(fields)) data.set(key, value);
  return data;
}

describe("librarySettingsOf", () => {
  it("normalises empty providers to null and missing modes to manual", () => {
    const settings = librarySettingsOf(
      library({
        metadata_provider: "",
        reading_status_push_mode: "",
        download_detection_mode: undefined as unknown as string,
        tags: undefined as unknown as string[],
      }),
    );
    expect(settings.metadata_provider).toBeNull();
    expect(settings.reading_status_push_mode).toBe("manual");
    expect(settings.download_detection_mode).toBe("manual");
    expect(settings.tags).toEqual([]);
  });
});

describe("librarySettingsFromForm", () => {
  it("reads checkboxes, selects and empty options", () => {
    const settings = librarySettingsFromForm(
      form({
        monitor_enabled: "true",
        scan_mode: "hourly",
        metadata_provider: "",
        fallback_metadata_provider: "google_books",
        metadata_refresh_mode: "daily",
        reading_status_provider: "anilist",
        reading_status_push_mode: "",
        download_detection_mode: "weekly",
      }),
      ["comics"],
    );
    expect(settings).toEqual({
      monitor_enabled: true,
      watcher_enabled: false,
      scan_mode: "hourly",
      metadata_refresh_mode: "daily",
      download_detection_mode: "weekly",
      metadata_provider: null,
      fallback_metadata_provider: "google_books",
      reading_status_provider: "anilist",
      reading_status_push_mode: "manual",
      tags: ["comics"],
    });
  });
});

describe("diffLibrarySettings", () => {
  it("produces no write for an unchanged form", () => {
    expect(diffLibrarySettings(before, { ...before })).toEqual([]);
  });

  it("ignores a tag reordering", () => {
    const b = { ...before, tags: ["manga", "comics"] };
    expect(diffLibrarySettings(b, { ...b, tags: ["comics", "manga"] })).toEqual([]);
  });

  it.each<[string, Partial<LibrarySettings>, LibraryWrite]>([
    [
      "monitoring",
      { watcher_enabled: true },
      {
        kind: "monitoring",
        monitor_enabled: true,
        scan_mode: "daily",
        watcher_enabled: true,
        metadata_refresh_mode: "weekly",
        download_detection_mode: "manual",
      },
    ],
    [
      "download detection (monitoring endpoint)",
      { download_detection_mode: "daily" },
      {
        kind: "monitoring",
        monitor_enabled: true,
        scan_mode: "daily",
        watcher_enabled: false,
        metadata_refresh_mode: "weekly",
        download_detection_mode: "daily",
      },
    ],
    [
      "metadata provider",
      { fallback_metadata_provider: "google_books" },
      { kind: "metadataProvider", provider: "anilist", fallbackProvider: "google_books" },
    ],
    [
      "reading status provider",
      { reading_status_provider: "anilist" },
      { kind: "readingStatusProvider", provider: "anilist", pushMode: "manual" },
    ],
    ["tags", { tags: ["manga", "webtoon"] }, { kind: "tags", tags: ["manga", "webtoon"] }],
  ])("only writes the %s section when it changed", (_name, change, expected) => {
    expect(diffLibrarySettings(before, { ...before, ...change })).toEqual([expected]);
  });

  it("orders writes monitoring, metadata, reading status, tags", () => {
    const writes = diffLibrarySettings(before, {
      ...before,
      tags: [],
      reading_status_push_mode: "daily",
      metadata_provider: null,
      scan_mode: "weekly",
    });
    expect(writes.map((w) => w.kind)).toEqual([
      "monitoring",
      "metadataProvider",
      "readingStatusProvider",
      "tags",
    ]);
  });
});

describe("applyLibraryWrites", () => {
  const writes: LibraryWrite[] = [
    { kind: "metadataProvider", provider: null, fallbackProvider: null },
    { kind: "readingStatusProvider", provider: null, pushMode: "manual" },
    { kind: "tags", tags: [] },
  ];

  it("applies writes sequentially, in order", async () => {
    const order: string[] = [];
    let inFlight = 0;
    const writer = vi.fn(async (_id: string, write: LibraryWrite) => {
      inFlight += 1;
      expect(inFlight).toBe(1);
      await Promise.resolve();
      order.push(write.kind);
      inFlight -= 1;
    });

    const result = await applyLibraryWrites("lib-1", writes, writer);

    expect(result).toEqual({
      status: "ok",
      applied: ["metadataProvider", "readingStatusProvider", "tags"],
    });
    expect(order).toEqual(["metadataProvider", "readingStatusProvider", "tags"]);
    expect(writer).toHaveBeenCalledWith("lib-1", writes[0]);
  });

  it("stops at the first failure and reports what was applied", async () => {
    const boom = new Error("boom");
    const writer = vi.fn(async (_id: string, write: LibraryWrite) => {
      if (write.kind === "readingStatusProvider") throw boom;
    });

    const result = await applyLibraryWrites("lib-1", writes, writer);

    expect(result).toEqual({
      status: "failed",
      applied: ["metadataProvider"],
      failed: "readingStatusProvider",
      error: boom,
    });
    expect(writer).toHaveBeenCalledTimes(2);
  });

  it("sends nothing for an empty diff", async () => {
    const writer = vi.fn();
    expect(await applyLibraryWrites("lib-1", [], writer)).toEqual({ status: "ok", applied: [] });
    expect(writer).not.toHaveBeenCalled();
  });
});
