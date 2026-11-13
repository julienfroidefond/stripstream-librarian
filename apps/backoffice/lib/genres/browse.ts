/**
 * Filter state of the genres page, without React (ADR 0028).
 *
 * The page sends one `GET /genres/browse` per filter change; this module turns
 * the UI selection into that query and owns the selection transitions
 * (include/exclude toggles, rename and delete of a genre).
 */

import type { GenreBrowseQuery } from "@/lib/api/genres";

export const GENRES_PAGE_SIZE = 100;

export type GenreFilterMode = "include" | "exclude";

/** `untagged` wins over `genres`/`exclude`, which are only kept for the "all" view. */
export type GenreSelection = {
  untagged: boolean;
  genres: string[];
  exclude: string[];
};

export const ALL_SERIES: GenreSelection = { untagged: false, genres: [], exclude: [] };
export const UNTAGGED_SERIES: GenreSelection = { untagged: true, genres: [], exclude: [] };

/** First view of the page: the series still waiting for a genre. */
export const INITIAL_GENRE_BROWSE: GenreBrowseQuery = { untagged: true };

export function browseQuery(
  selection: GenreSelection,
  libraryIds: readonly string[],
  q: string,
  page = 1,
): GenreBrowseQuery {
  return {
    ...(selection.untagged
      ? { untagged: true }
      : { genres: selection.genres, exclude: selection.exclude }),
    libraryIds,
    q: q.trim() || undefined,
    page,
    limit: GENRES_PAGE_SIZE,
  };
}

export function isAllSeries(selection: GenreSelection): boolean {
  return !selection.untagged && selection.genres.length === 0 && selection.exclude.length === 0;
}

function toggle(values: string[], value: string): string[] {
  return values.includes(value) ? values.filter(v => v !== value) : [...values, value];
}

/** Click on a genre chip: add/remove it from the included or excluded list. */
export function toggleGenre(selection: GenreSelection, genre: string, mode: GenreFilterMode): GenreSelection {
  const base = selection.untagged ? ALL_SERIES : selection;
  return mode === "include"
    ? { untagged: false, genres: toggle(base.genres, genre), exclude: base.exclude.filter(g => g !== genre) }
    : { untagged: false, genres: base.genres.filter(g => g !== genre), exclude: toggle(base.exclude, genre) };
}

export function renameGenreInSelection(selection: GenreSelection, from: string, to: string): GenreSelection {
  const rename = (values: string[]) => Array.from(new Set(values.map(v => (v === from ? to : v))));
  return { ...selection, genres: rename(selection.genres), exclude: rename(selection.exclude) };
}

export function removeGenreFromSelection(selection: GenreSelection, genre: string): GenreSelection {
  return {
    ...selection,
    genres: selection.genres.filter(g => g !== genre),
    exclude: selection.exclude.filter(g => g !== genre),
  };
}
