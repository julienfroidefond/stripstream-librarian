/** Browser-side genre management. */

import type { GenreBrowsePage, GenreBrowseQuery, GenreDto } from "../genres";
import type { SeriesDto } from "../series";
import { browserFetch, browserFetchVoid, withQuery } from "./transport";

export type { GenreBrowsePage, GenreBrowseQuery, GenreCover, GenreDto, GenreFacet } from "../genres";

/**
 * `path` + query string of a genre browse (repeated keys, empty filters omitted).
 * Shared by the browser client and the server loader so both send the same query.
 */
export function genreBrowsePath(path: string, query: GenreBrowseQuery): string {
  return withQuery(path, {
    genres: query.genres?.length ? query.genres : undefined,
    exclude: query.exclude?.length ? query.exclude : undefined,
    library_ids: query.libraryIds?.length ? query.libraryIds : undefined,
    untagged: query.untagged ? "true" : undefined,
    q: query.q?.trim() || undefined,
    page: query.page,
    limit: query.limit,
  });
}

export type GenreSuggestion = {
  series_id: string;
  name: string;
  tags: string[];
};

export const genresApi = {
  list(libraryId?: string): Promise<GenreDto[]> {
    return browserFetch<GenreDto[]>(withQuery("/api/genres", { library_id: libraryId }));
  },

  /** Series of the genres page in one request: filtered page + library-scoped facets. */
  browse(query: GenreBrowseQuery, signal?: AbortSignal): Promise<GenreBrowsePage> {
    return browserFetch<GenreBrowsePage>(genreBrowsePath("/api/genres/browse", query), { signal });
  },

  untagged(libraryId?: string, q?: string): Promise<SeriesDto[]> {
    return browserFetch<SeriesDto[]>(
      withQuery("/api/genres/untagged-series", { library_id: libraryId, q })
    );
  },

  rename(oldName: string, newName: string): Promise<void> {
    return browserFetchVoid(`/api/genres/${encodeURIComponent(oldName)}`, {
      method: "PATCH",
      body: { new_name: newName },
    });
  },

  remove(name: string): Promise<void> {
    return browserFetchVoid(`/api/genres/${encodeURIComponent(name)}`, { method: "DELETE" });
  },

  assign(genre: string, seriesIds: string[]): Promise<void> {
    return browserFetchVoid("/api/genres/assign", {
      method: "POST",
      body: { genre, series_ids: seriesIds },
    });
  },

  aiSuggest(seriesIds: string[]): Promise<{ suggestions: GenreSuggestion[] }> {
    return browserFetch<{ suggestions: GenreSuggestion[] }>("/api/genres/ai-suggest", {
      method: "POST",
      body: { series_ids: seriesIds },
    });
  },
};
