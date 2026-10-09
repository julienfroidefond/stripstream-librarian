import type { GapSummaryDto } from "./api";
import type { IconName } from "@/app/components/ui";
import type { TranslationKey } from "./i18n/fr";

export type GapTab = "series" | "books";

export interface GapDef {
  value: string;
  labelKey: TranslationKey;
  countKey: keyof GapSummaryDto;
  icon: IconName;
}

export const SERIES_GAPS: readonly GapDef[] = [
  { value: "no_description", labelKey: "metadata.gap.noDescription", countKey: "series_no_description", icon: "document" },
  { value: "no_genre", labelKey: "metadata.gap.noGenre", countKey: "series_no_genre", icon: "tag" },
  { value: "no_authors", labelKey: "metadata.gap.noAuthor", countKey: "series_no_authors", icon: "authors" },
  { value: "no_publishers", labelKey: "metadata.gap.noPublishers", countKey: "series_no_publishers", icon: "building" },
  { value: "no_year", labelKey: "metadata.gap.noYear", countKey: "series_no_year", icon: "calendar" },
  { value: "no_cover", labelKey: "metadata.gap.noCover", countKey: "series_no_cover", icon: "image" },
  { value: "no_community_score", labelKey: "metadata.gap.noCommunityScore", countKey: "series_no_community_score", icon: "star" },
];

export const BOOKS_GAPS: readonly GapDef[] = [
  { value: "no_summary", labelKey: "metadata.gap.noSummary", countKey: "books_no_summary", icon: "document" },
  { value: "no_isbn", labelKey: "metadata.gap.noIsbn", countKey: "books_no_isbn", icon: "hash" },
  { value: "no_cover", labelKey: "metadata.gap.noCover", countKey: "books_no_cover", icon: "image" },
  { value: "no_author", labelKey: "metadata.gap.noAuthor", countKey: "books_no_author", icon: "authors" },
  { value: "no_publish_date", labelKey: "metadata.gap.noPublishDate", countKey: "books_no_publish_date", icon: "calendar" },
  { value: "no_language", labelKey: "metadata.gap.noLanguage", countKey: "books_no_language", icon: "globe" },
  { value: "no_volume", labelKey: "metadata.gap.noVolume", countKey: "books_no_volume", icon: "number" },
];

export function isGapTab(value: string | undefined): value is GapTab {
  return value === "series" || value === "books";
}

export function gapsForTab(tab: GapTab): readonly GapDef[] {
  return tab === "books" ? BOOKS_GAPS : SERIES_GAPS;
}

export function totalCountKey(tab: GapTab): keyof GapSummaryDto {
  return tab === "books" ? "books_total" : "series_total";
}

export function isValidGap(tab: GapTab, value: string | undefined): boolean {
  if (!value) return false;
  return gapsForTab(tab).some((gap) => gap.value === value);
}
