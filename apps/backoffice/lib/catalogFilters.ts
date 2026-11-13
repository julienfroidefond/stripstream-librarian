/**
 * Catalog filter vocabulary shared by the series, library series, books and
 * metadata pages: the option tables of `LiveSearchForm` selects, the series
 * status labels and the "is any filter active" rule. Pure (no React, no API).
 */

import type { TranslateFunction } from "./i18n/dictionaries";

export type FilterOption = { value: string; label: string };

/** Series publication statuses the UI knows how to label. */
export const KNOWN_SERIES_STATUSES = ["ongoing", "ended", "hiatus", "cancelled", "upcoming"] as const;

/** Status → translated label; unknown statuses are displayed raw by the callers. */
export function seriesStatusLabels(t: TranslateFunction): Record<string, string> {
  return Object.fromEntries(
    KNOWN_SERIES_STATUSES.map((status) => [status, t(`seriesStatus.${status}`)]),
  );
}

/** Options for the series status select, from the statuses present in the database. */
export function seriesStatusOptions(t: TranslateFunction, dbStatuses: string[]): FilterOption[] {
  const labels = seriesStatusLabels(t);
  return [
    { value: "", label: t("seriesStatus.allStatuses") },
    ...dbStatuses.map((status) => ({ value: status, label: labels[status] || status })),
  ];
}

export function readingStatusOptions(t: TranslateFunction): FilterOption[] {
  return [
    { value: "", label: t("common.all") },
    { value: "unread", label: t("status.unread") },
    { value: "reading", label: t("status.reading") },
    { value: "read", label: t("status.read") },
  ];
}

export function libraryOptions(
  t: TranslateFunction,
  libraries: ReadonlyArray<{ id: string; name: string }>,
): FilterOption[] {
  return [
    { value: "", label: t("books.allLibraries") },
    ...libraries.map((library) => ({ value: library.id, label: library.name })),
  ];
}

export function missingBooksOptions(t: TranslateFunction): FilterOption[] {
  return [
    { value: "", label: t("common.all") },
    { value: "true", label: t("series.missingBooks") },
  ];
}

/** Linked / unlinked, then one entry per provider (when the page filters by provider). */
export function metadataOptions(
  t: TranslateFunction,
  providers: ReadonlyArray<{ id: string; label: string }> = [],
): FilterOption[] {
  return [
    { value: "", label: t("series.metadataAll") },
    { value: "linked", label: t("series.metadataLinked") },
    { value: "unlinked", label: t("series.metadataUnlinked") },
    ...providers.map((provider) => ({ value: provider.id, label: provider.label })),
  ];
}

export type CatalogSortKind = "books" | "librarySeries" | "series";

/** Title / latest for every page; series add release date, the global list community score. */
export function sortOptions(t: TranslateFunction, kind: CatalogSortKind): FilterOption[] {
  return [
    { value: "", label: t("books.sortTitle") },
    { value: "latest", label: t("books.sortLatest") },
    ...(kind !== "books" ? [{ value: "release_date", label: t("series.sortReleaseDate") }] : []),
    ...(kind === "series" ? [{ value: "community_score", label: t("series.sortCommunityScore") }] : []),
  ];
}

/**
 * A filter is active when one of the form's fields has a non-blank value.
 * `LiveSearchForm` uses it for its "clear" button; pages use it, on the values
 * they pass to the form, to pick the empty-state message.
 */
export function hasActiveFilters(
  values: Readonly<Record<string, string | undefined>>,
  fields: ReadonlyArray<{ name: string }>,
): boolean {
  return fields.some((field) => (values[field.name] ?? "").trim() !== "");
}
