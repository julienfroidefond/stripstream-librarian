/** Browser-side genre management. */

import type { SeriesDto } from "../series";
import { browserFetch, browserFetchVoid, withQuery } from "./transport";

export type GenreDto = {
  name: string;
  series_count: number;
};

export type GenreSuggestion = {
  series_id: string;
  name: string;
  tags: string[];
};

export const genresApi = {
  list(libraryId?: string): Promise<GenreDto[]> {
    return browserFetch<GenreDto[]>(withQuery("/api/genres", { library_id: libraryId }));
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
