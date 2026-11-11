import { describe, expect, it } from "vitest";

import type { AvailableReleaseDto, LatestFoundPerLibraryDto } from "@/lib/api/downloads";
import type { TelegramAvailableGroupDto } from "@/lib/api/telegram";
import {
  availableLibraries,
  availableSourceVolumes,
  bestSeeders,
  buildAvailableGroups,
  filterAvailableGroups,
  sortAvailableGroups,
  summarizeAvailableSources,
  visibleAvailableSources,
  type AvailableSource,
} from "@/lib/downloads/available";

function release(overrides: Partial<AvailableReleaseDto> = {}): AvailableReleaseDto {
  return {
    title: "Berserk - T3",
    size: 1024,
    download_url: "https://indexer/1",
    indexer: "IndexerA",
    seeders: 12,
    matched_missing_volumes: [3],
    all_volumes: [3],
    has_failed: false,
    ...overrides,
  };
}

function latestFound(
  overrides: Partial<LatestFoundPerLibraryDto["results"][number]> = {},
): LatestFoundPerLibraryDto {
  return {
    library_id: "lib-1",
    library_name: "BD",
    results: [
      {
        id: "entry-1",
        series_id: "series-1",
        series_name: "Berserk",
        missing_count: 1,
        available_releases: [release()],
        updated_at: "2026-01-01T00:00:00Z",
        failed_download_count: 0,
        ...overrides,
      },
    ],
  };
}

function telegramGroup(
  overrides: Partial<TelegramAvailableGroupDto> = {},
): TelegramAvailableGroupDto {
  return {
    series_name: "Berserk",
    series_id: "series-1",
    library_id: "lib-1",
    library_name: "BD",
    owned_volumes: [],
    series_missing_count: 2,
    books: [
      {
        id: "tg-1",
        channel_username: "channel",
        message_id: 1,
        filename: "Berserk - T2.cbz",
        file_size: 100,
        volume_number: 2,
        status: "available",
        created_at: "2026-01-02T00:00:00Z",
      },
    ],
    ...overrides,
  };
}

describe("buildAvailableGroups", () => {
  it("classifies releases by provider and uses the newest detection date", () => {
    const groups = buildAvailableGroups(
      [
        latestFound({
          available_releases: [
            release({ provider: null, detected_at: "2026-01-02T00:00:00Z" }),
            release({
              provider: "fourtoutici",
              external_id: "ext-3",
              detected_at: "2026-01-03T00:00:00Z",
            }),
          ],
        }),
      ],
      [],
    );

    expect(groups).toHaveLength(1);
    expect(groups[0].sources.map((source) => source.kind)).toEqual(["prowlarr", "ddl"]);
    expect(groups[0].updated_at).toBe("2026-01-03T00:00:00Z");
  });

  it("drops detection results without releases", () => {
    const groups = buildAvailableGroups(
      [latestFound({ available_releases: [] })],
      [],
    );

    expect(groups).toEqual([]);
  });

  it("merges Telegram books into the catalogue group and unions owned volumes", () => {
    const groups = buildAvailableGroups(
      [latestFound({ available_releases: [release()] })],
      [
        telegramGroup({
          owned_volumes: [1],
          series_missing_count: 3,
          books: [
            {
              id: "tg-1",
              channel_username: "channel",
              message_id: 1,
              filename: "Berserk - T2.cbz",
              file_size: 100,
              volume_number: 2,
              status: "available",
              created_at: "2026-01-04T00:00:00Z",
            },
            {
              id: "tg-owned",
              channel_username: "channel",
              message_id: 2,
              filename: "Berserk - T1.cbz",
              file_size: 100,
              volume_number: 1,
              status: "available",
              created_at: "2026-01-05T00:00:00Z",
            },
          ],
        }),
      ],
    );

    expect(groups).toHaveLength(1);
    expect(groups[0].missing_count).toBe(3);
    expect(groups[0].owned_volumes).toEqual([1]);
    expect(groups[0].updated_at).toBe("2026-01-04T00:00:00Z");
    expect(groups[0].sources.map((source) => source.kind)).toEqual(["prowlarr", "telegram"]);
  });

  it("deduplicates Telegram books already present in another source", () => {
    const book = {
      id: "tg-1",
      channel_username: "channel",
      message_id: 1,
      filename: "Berserk - T2.cbz",
      file_size: 100,
      volume_number: 2,
      status: "available",
      created_at: "2026-01-02T00:00:00Z",
    };

    const groups = buildAvailableGroups(
      [],
      [
        telegramGroup({ books: [book] }),
        telegramGroup({ books: [book], series_missing_count: 1 }),
      ],
    );

    expect(groups).toHaveLength(1);
    expect(groups[0].sources).toHaveLength(1);
  });

  it("drops a fully owned Telegram group", () => {
    const groups = buildAvailableGroups(
      [],
      [
        telegramGroup({
          owned_volumes: [2],
          series_missing_count: 0,
          books: [],
        }),
      ],
    );

    expect(groups).toEqual([]);
  });

  it("ignores Telegram books whose volumes are already owned", () => {
    const groups = buildAvailableGroups(
      [],
      [
        telegramGroup({
          owned_volumes: [2],
          books: [
            {
              id: "tg-owned",
              channel_username: "channel",
              message_id: 1,
              filename: "Berserk - T2.cbz",
              file_size: 100,
              volume_number: 2,
              status: "available",
              created_at: "2026-01-02T00:00:00Z",
            },
          ],
        }),
      ],
    );

    expect(groups).toEqual([]);
  });
});

describe("availableSourceVolumes", () => {
  it("reads Telegram volumes and release missing volumes", () => {
    const telegram: AvailableSource = {
      kind: "telegram",
      book: {
        id: "tg-1",
        channel_username: "channel",
        message_id: 1,
        filename: "Berserk - T2.cbz",
        file_size: 100,
        volume_number: 2,
        status: "available",
        created_at: "2026-01-02T00:00:00Z",
      },
    };
    const prowlarr: AvailableSource = {
      kind: "prowlarr",
      entryId: "entry-1",
      releaseIndex: 0,
      release: release({ matched_missing_volumes: [3, 4] }),
      detectedAt: "2026-01-01T00:00:00Z",
    };

    expect(availableSourceVolumes(telegram)).toEqual([2]);
    expect(availableSourceVolumes(prowlarr)).toEqual([3, 4]);
  });
});

describe("filterAvailableGroups", () => {
  it("filters by library and source", () => {
    const groups = buildAvailableGroups(
      [latestFound({ available_releases: [release()] })],
      [telegramGroup()],
    );

    expect(filterAvailableGroups(groups, { libraryId: "lib-2" })).toEqual([]);
    expect(filterAvailableGroups(groups, { source: "ddl" })).toEqual([]);
    expect(filterAvailableGroups(groups, { source: "telegram" })).toHaveLength(1);
    expect(filterAvailableGroups(groups)).toHaveLength(1);
  });

  it("deduplicates libraries across groups", () => {
    const groups = buildAvailableGroups(
      [
        latestFound(),
        latestFound({
          id: "entry-2",
          series_id: "series-2",
          series_name: "One Piece",
          available_releases: [release({ title: "One Piece - T1" })],
        }),
        {
          library_id: "lib-2",
          library_name: "Manga",
          results: [
            {
              id: "entry-3",
              series_id: "series-3",
              series_name: "Naruto",
              missing_count: 1,
              available_releases: [release({ title: "Naruto - T1" })],
              updated_at: "2026-01-01T00:00:00Z",
              failed_download_count: 0,
            },
          ],
        },
      ],
      [],
    );

    expect(availableLibraries(groups)).toEqual([
      { id: "lib-1", name: "BD" },
      { id: "lib-2", name: "Manga" },
    ]);
  });
});

describe("summarizeAvailableSources", () => {
  it("counts sources, failures and the best Prowlarr seeder count", () => {
    const sources: AvailableSource[] = [
      {
        kind: "prowlarr",
        entryId: "entry-1",
        releaseIndex: 0,
        release: release({ seeders: 4, has_failed: true }),
        detectedAt: "2026-01-01T00:00:00Z",
      },
      {
        kind: "prowlarr",
        entryId: "entry-2",
        releaseIndex: 0,
        release: release({ seeders: 18 }),
        detectedAt: "2026-01-01T00:00:00Z",
      },
      {
        kind: "ddl",
        entryId: "entry-3",
        releaseIndex: 0,
        release: release({ provider: "fourtoutici", seeders: 99 }),
        detectedAt: "2026-01-01T00:00:00Z",
      },
      {
        kind: "telegram",
        book: {
          id: "tg-1",
          channel_username: "channel",
          message_id: 1,
          filename: "Berserk - T2.cbz",
          file_size: 100,
          volume_number: 2,
          status: "failed",
          created_at: "2026-01-02T00:00:00Z",
        },
      },
    ];

    expect(bestSeeders(sources)).toBe(18);
    expect(summarizeAvailableSources(sources)).toEqual({
      prowlarr: 2,
      ddl: 1,
      telegram: 1,
      failed: 2,
      seeders: 18,
    });
  });

  it("keeps only the requested source kind", () => {
    const groups = buildAvailableGroups(
      [latestFound({ available_releases: [release()] })],
      [telegramGroup()],
    );

    expect(visibleAvailableSources(groups[0], "telegram").map((source) => source.kind)).toEqual([
      "telegram",
    ]);
    expect(visibleAvailableSources(groups[0], "all")).toHaveLength(2);
  });
});

describe("sortAvailableGroups", () => {
  it("sorts by seeders, missing count, name and recency", () => {
    const groups = buildAvailableGroups(
      [
        latestFound({
          series_name: "Zeta",
          missing_count: 1,
          updated_at: "2026-01-03T00:00:00Z",
        }),
        latestFound({
          id: "entry-2",
          series_id: "series-2",
          series_name: "Alpha",
          missing_count: 5,
          available_releases: [release({ seeders: 100 })],
          updated_at: "2026-01-02T00:00:00Z",
        }),
      ],
      [],
    );

    expect(sortAvailableGroups(groups, "seeders").map((group) => group.series_name)).toEqual([
      "Alpha",
      "Zeta",
    ]);
    expect(sortAvailableGroups(groups, "missing").map((group) => group.series_name)).toEqual([
      "Alpha",
      "Zeta",
    ]);
    expect(sortAvailableGroups(groups, "name").map((group) => group.series_name)).toEqual([
      "Alpha",
      "Zeta",
    ]);
    expect(sortAvailableGroups(groups, "recent").map((group) => group.series_name)).toEqual([
      "Zeta",
      "Alpha",
    ]);
  });
});
