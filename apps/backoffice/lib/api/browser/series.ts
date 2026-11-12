/** Browser-side series catalog, mutations and admin views. */

import type {
  ArchivedSeriesDetailDto,
  ArchivedSeriesItemDto,
} from "../archive";
import type {
  OrphanDeleteResultDto,
  OrphanSeriesPageDto,
  SeriesPageDto,
} from "../series";
import { browserFetch, browserFetchVoid, withQuery } from "./transport";

export interface SeriesListQuery {
  genre?: string;
  libraryId?: string;
  q?: string;
  page?: number;
  limit?: number;
  noBooks?: boolean;
  hasBooks?: boolean;
}

export interface SeriesCreateRequest {
  library_id: string;
  name: string;
  provider?: string;
  external_id?: string;
  external_url?: string | null;
  confidence?: number;
  total_volumes?: number | null;
  metadata_json?: Record<string, unknown>;
}

/** Fields accepted by `PATCH /series/:id`; all are optional per edit form. */
export interface SeriesUpdateRequest {
  new_name?: string;
  authors?: string[];
  author?: string | null;
  language?: string | null;
  genres?: string[];
  description?: string | null;
  publishers?: string[];
  start_year?: number | null;
  total_volumes?: number | null;
  status?: string | null;
  locked_fields?: Record<string, boolean>;
}

export interface RenameBookOverride {
  book_id: string;
  volume: number | null;
  volume_type: string | null;
}

export interface RenameBooksRequest {
  format: string;
  format_hs: string;
  format_int: string;
  format_oneshot: string;
  mode: "preview" | "execute";
  overrides: RenameBookOverride[];
}

export interface RenameBookEntry {
  book_id: string;
  old_filename: string;
  new_filename: string;
  old_path: string;
  new_path: string;
  changed: boolean;
  volume: number | null;
  volume_type: string;
}

export interface RenameBooksResponse {
  series_id: string;
  renames: RenameBookEntry[];
  errors: { book_id: string; filename: string; error: string }[];
  executed: boolean;
}

function listPath(query: SeriesListQuery): string {
  return withQuery("/api/series", {
    genre: query.genre,
    library_id: query.libraryId,
    q: query.q,
    page: query.page,
    limit: query.limit,
    no_books: query.noBooks ? "true" : undefined,
    has_books: query.hasBooks ? "true" : undefined,
  });
}

export const seriesApi = {
  list(query: SeriesListQuery = {}): Promise<SeriesPageDto> {
    return browserFetch<SeriesPageDto>(listPath(query));
  },

  async count(query: Omit<SeriesListQuery, "limit" | "page"> = {}): Promise<number> {
    const page = await browserFetch<SeriesPageDto>(listPath({ ...query, limit: 1 }));
    return page.total;
  },

  search(q: string, page = 1, limit = 20): Promise<SeriesPageDto> {
    return browserFetch<SeriesPageDto>(
      withQuery("/api/series/search", { q, page, limit })
    );
  },

  create(body: SeriesCreateRequest): Promise<{ series_id: string }> {
    return browserFetch<{ series_id: string }>("/api/series/create", {
      method: "POST",
      body,
    });
  },

  update(seriesId: string, body: SeriesUpdateRequest): Promise<{ updated: number }> {
    return browserFetch<{ updated: number }>(`/api/series/${encodeURIComponent(seriesId)}`, {
      method: "PATCH",
      body,
    });
  },

  remove(seriesId: string): Promise<void> {
    return browserFetchVoid(`/api/series/${encodeURIComponent(seriesId)}`, { method: "DELETE" });
  },

  merge(
    targetId: string,
    sourceId: string
  ): Promise<{ books_moved: number; metadata_moved: number; downloads_moved: number }> {
    return browserFetch(`/api/series/${encodeURIComponent(targetId)}/merge`, {
      method: "POST",
      body: { source_id: sourceId },
    });
  },

  renameBooks(seriesId: string, body: RenameBooksRequest): Promise<RenameBooksResponse> {
    return browserFetch<RenameBooksResponse>(
      `/api/series/${encodeURIComponent(seriesId)}/rename-books`,
      { method: "POST", body }
    );
  },

  setShelf(seriesId: string, shelved: boolean): Promise<void> {
    return browserFetchVoid(`/api/series/${encodeURIComponent(seriesId)}/shelf`, {
      method: shelved ? "PUT" : "DELETE",
    });
  },

  markRead(seriesId: string, status: "read" | "unread"): Promise<{ updated: number }> {
    return browserFetch<{ updated: number }>("/api/series/mark-read", {
      method: "POST",
      body: { series: seriesId, status },
    });
  },

  updateRating(seriesId: string, rating: number): Promise<void> {
    return browserFetchVoid(`/api/series/${encodeURIComponent(seriesId)}/rating`, {
      method: "PUT",
      body: { rating },
    });
  },

  deleteRating(seriesId: string): Promise<void> {
    return browserFetchVoid(`/api/series/${encodeURIComponent(seriesId)}/rating`, {
      method: "DELETE",
    });
  },

  /** Distinct genre names currently attached to series. */
  genres(): Promise<string[]> {
    return browserFetch<string[]>("/api/series/genres");
  },

  archived(): Promise<ArchivedSeriesItemDto[]> {
    return browserFetch<ArchivedSeriesItemDto[]>("/api/series/archived");
  },

  archivedDetail(seriesId: string): Promise<ArchivedSeriesDetailDto> {
    return browserFetch<ArchivedSeriesDetailDto>(
      `/api/series/archived/${encodeURIComponent(seriesId)}`
    );
  },

  orphans(page = 1, limit = 50): Promise<OrphanSeriesPageDto> {
    return browserFetch<OrphanSeriesPageDto>(
      withQuery("/api/series/orphans", { page, limit })
    );
  },

  deleteOrphans(ids: string[]): Promise<OrphanDeleteResultDto> {
    return browserFetch<OrphanDeleteResultDto>("/api/series/orphans", {
      method: "DELETE",
      body: { ids },
    });
  },
};
