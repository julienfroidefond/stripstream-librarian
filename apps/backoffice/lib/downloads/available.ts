/**
 * Read model for the "available downloads" section.
 *
 * Prowlarr/DDL releases and Telegram books arrive as separate lists and are
 * grouped per series so the page can render one row per series with a unified
 * source count, missing-volume count and best seeder count.
 */

import type { AvailableReleaseDto, LatestFoundPerLibraryDto } from "@/lib/api/downloads";
import type {
  TelegramAvailableBookDto,
  TelegramAvailableGroupDto,
} from "@/lib/api/telegram";

export type AvailableSource =
  | {
      kind: "prowlarr";
      entryId: string;
      releaseIndex: number;
      release: AvailableReleaseDto;
      detectedAt: string;
    }
  | {
      kind: "ddl";
      entryId: string;
      releaseIndex: number;
      release: AvailableReleaseDto;
      detectedAt: string;
    }
  | {
      kind: "telegram";
      book: TelegramAvailableBookDto;
    };

export type AvailableGroup = {
  key: string;
  series_id: string | null;
  series_name: string;
  library_id: string;
  library_name: string;
  missing_count: number;
  owned_volumes: number[];
  updated_at: string;
  sources: AvailableSource[];
};

export type AvailableSortKey = "seeders" | "missing" | "name" | "recent";
export type AvailableSourceFilter = "all" | "prowlarr" | "ddl" | "telegram";

export type AvailableSourceSummary = {
  prowlarr: number;
  ddl: number;
  telegram: number;
  failed: number;
  seeders: number;
};

function newestDate(a: string, b: string): string {
  return a.localeCompare(b) >= 0 ? a : b;
}

function availableGroupKey(libraryId: string, seriesId: string | null, seriesName: string): string {
  return `${libraryId}|${seriesId ?? seriesName.toLocaleLowerCase()}`;
}

export function availableSourceVolumes(source: AvailableSource): number[] {
  if (source.kind === "telegram") {
    return source.book.volume_number == null ? [] : [source.book.volume_number];
  }
  return source.release.matched_missing_volumes;
}

/**
 * Group releases and Telegram books per series/library.
 *
 * Telegram books already owned by the library are dropped, and groups with no
 * remaining source are discarded. Series identity follows the catalogue when
 * `series_id` is known, otherwise the accent/case-insensitive series name.
 */
export function buildAvailableGroups(
  latestFound: LatestFoundPerLibraryDto[],
  telegramAvailable: TelegramAvailableGroupDto[],
): AvailableGroup[] {
  const groups = new Map<string, AvailableGroup>();

  const ensureGroup = (
    libraryId: string,
    libraryName: string,
    seriesId: string | null,
    seriesName: string,
    missingCount: number,
    ownedVolumes: number[],
    updatedAt: string,
  ) => {
    const key = availableGroupKey(libraryId, seriesId, seriesName);
    const existing = groups.get(key);
    if (existing) {
      existing.missing_count = Math.max(existing.missing_count, missingCount);
      existing.updated_at = newestDate(existing.updated_at, updatedAt);
      existing.owned_volumes = Array.from(new Set([...existing.owned_volumes, ...ownedVolumes]));
      return existing;
    }

    const group: AvailableGroup = {
      key,
      series_id: seriesId,
      series_name: seriesName,
      library_id: libraryId,
      library_name: libraryName,
      missing_count: missingCount,
      owned_volumes: ownedVolumes,
      updated_at: updatedAt,
      sources: [],
    };
    groups.set(key, group);
    return group;
  };

  latestFound.forEach(library => {
    library.results.forEach(result => {
      const releases = result.available_releases ?? [];
      if (releases.length === 0) return;
      const group = ensureGroup(
        library.library_id,
        library.library_name,
        result.series_id,
        result.series_name,
        result.missing_count,
        [],
        result.updated_at,
      );
      releases.forEach((release, releaseIndex) => {
        const detectedAt = release.detected_at ?? result.updated_at;
        const kind = release.provider ? "ddl" : "prowlarr";
        group.sources.push({ kind, entryId: result.id, releaseIndex, release, detectedAt });
        group.updated_at = newestDate(group.updated_at, detectedAt);
      });
    });
  });

  telegramAvailable.forEach(telegramGroup => {
    const owned = new Set(telegramGroup.owned_volumes);
    const books = telegramGroup.books.filter(
      book => !(book.volume_number != null && owned.has(book.volume_number)),
    );
    if (books.length === 0) return;
    if (telegramGroup.series_missing_count === 0 && telegramGroup.owned_volumes.length > 0) return;

    const newestBookDate = books.reduce(
      (latest, book) => newestDate(latest, book.created_at),
      books[0]?.created_at ?? "",
    );
    const group = ensureGroup(
      telegramGroup.library_id,
      telegramGroup.library_name,
      telegramGroup.series_id,
      telegramGroup.series_name,
      telegramGroup.series_missing_count,
      telegramGroup.owned_volumes,
      newestBookDate,
    );

    const existingTelegramIds = new Set(
      group.sources
        .filter((source): source is Extract<AvailableSource, { kind: "telegram" }> =>
          source.kind === "telegram",
        )
        .map(source => source.book.id),
    );
    books.forEach(book => {
      if (!existingTelegramIds.has(book.id)) {
        group.sources.push({ kind: "telegram", book });
      }
    });
  });

  return Array.from(groups.values()).filter(group => group.sources.length > 0);
}

export function visibleAvailableSources(
  group: AvailableGroup,
  sourceFilter: AvailableSourceFilter,
): AvailableSource[] {
  if (sourceFilter === "all") return group.sources;
  return group.sources.filter(source => source.kind === sourceFilter);
}

export function filterAvailableGroups(
  groups: AvailableGroup[],
  {
    libraryId = "all",
    source = "all",
  }: { libraryId?: string; source?: AvailableSourceFilter } = {},
): AvailableGroup[] {
  return groups.filter(group => {
    if (libraryId !== "all" && group.library_id !== libraryId) return false;
    if (source !== "all" && !group.sources.some(item => item.kind === source)) return false;
    return true;
  });
}

export function availableLibraries(groups: AvailableGroup[]): Array<{ id: string; name: string }> {
  return Array.from(new Map(groups.map(group => [group.library_id, group.library_name])).entries())
    .map(([id, name]) => ({ id, name }));
}

/** Best Prowlarr seeder count; Telegram and DDL sources do not carry seeders. */
export function bestSeeders(sources: AvailableSource[]): number {
  return sources.reduce((max, source) => {
    if (source.kind !== "prowlarr") return max;
    return Math.max(max, source.release.seeders ?? 0);
  }, 0);
}

export function summarizeAvailableSources(sources: AvailableSource[]): AvailableSourceSummary {
  const summary: AvailableSourceSummary = {
    prowlarr: 0,
    ddl: 0,
    telegram: 0,
    failed: 0,
    seeders: bestSeeders(sources),
  };

  sources.forEach(source => {
    if (source.kind === "telegram") {
      summary.telegram += 1;
      if (source.book.status === "failed") summary.failed += 1;
      return;
    }
    summary[source.kind] += 1;
    if (source.release.has_failed) summary.failed += 1;
  });

  return summary;
}

export function sortAvailableGroups(
  groups: AvailableGroup[],
  sort: AvailableSortKey,
): AvailableGroup[] {
  return [...groups].sort((a, b) => {
    switch (sort) {
      case "seeders":
        return bestSeeders(b.sources) - bestSeeders(a.sources);
      case "missing":
        return b.missing_count - a.missing_count;
      case "name":
        return a.series_name.localeCompare(b.series_name);
      case "recent":
        return b.updated_at.localeCompare(a.updated_at);
      default:
        return 0;
    }
  });
}
