import { describe, expect, it } from "vitest";

import {
  hasActiveFilters,
  libraryOptions,
  metadataOptions,
  missingBooksOptions,
  readingStatusOptions,
  seriesStatusLabels,
  seriesStatusOptions,
  sortOptions,
} from "@/lib/catalogFilters";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";

const t = ((key: string) => key) as TranslateFunction;
const values = (options: Array<{ value: string }>) => options.map((option) => option.value);

describe("catalog filter options", () => {
  it("labels the known series statuses", () => {
    expect(seriesStatusLabels(t)).toEqual({
      ongoing: "seriesStatus.ongoing",
      ended: "seriesStatus.ended",
      hiatus: "seriesStatus.hiatus",
      cancelled: "seriesStatus.cancelled",
      upcoming: "seriesStatus.upcoming",
    });
  });

  it("builds series status options from the database, raw label when unknown", () => {
    expect(seriesStatusOptions(t, ["ended", "abandoned"])).toEqual([
      { value: "", label: "seriesStatus.allStatuses" },
      { value: "ended", label: "seriesStatus.ended" },
      { value: "abandoned", label: "abandoned" },
    ]);
  });

  it("varies the sort options by page", () => {
    expect(values(sortOptions(t, "books"))).toEqual(["", "latest"]);
    expect(values(sortOptions(t, "librarySeries"))).toEqual(["", "latest", "release_date"]);
    expect(values(sortOptions(t, "series"))).toEqual(["", "latest", "release_date", "community_score"]);
  });

  it("appends providers to the metadata options", () => {
    expect(values(metadataOptions(t))).toEqual(["", "linked", "unlinked"]);
    expect(metadataOptions(t, [{ id: "anilist", label: "AniList" }]).at(-1)).toEqual({
      value: "anilist",
      label: "AniList",
    });
  });

  it("builds the library, reading status and missing options", () => {
    expect(libraryOptions(t, [{ id: "l1", name: "Manga" }])).toEqual([
      { value: "", label: "books.allLibraries" },
      { value: "l1", label: "Manga" },
    ]);
    expect(values(readingStatusOptions(t))).toEqual(["", "unread", "reading", "read"]);
    expect(values(missingBooksOptions(t))).toEqual(["", "true"]);
  });
});

describe("hasActiveFilters", () => {
  const fields = [{ name: "q" }, { name: "sort" }];

  it("is false when every field is blank", () => {
    expect(hasActiveFilters({ q: "  ", sort: "" }, fields)).toBe(false);
    expect(hasActiveFilters({}, fields)).toBe(false);
  });

  it("is true when a field has a value", () => {
    expect(hasActiveFilters({ q: "", sort: "latest" }, fields)).toBe(true);
  });

  it("ignores values of fields the form does not show", () => {
    expect(hasActiveFilters({ status: "read" }, fields)).toBe(false);
  });
});
