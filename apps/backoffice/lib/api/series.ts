/**
 * Séries : recherche, métadonnées, notes, genres, fusion.
 */

import { apiFetch } from "./client";
import { cacheTags } from "../cache/tags";

export type SeriesDto = {
  series_id: string;
  name: string;
  book_count: number;
  books_read_count: number;
  last_read_at: string | null;
  first_book_id: string | null;
  first_book_updated_at: string | null;
  library_id: string;
  series_status: string | null;
  missing_count: number | null;
  metadata_provider: string | null;
  anilist_id: number | null;
  anilist_url: string | null;
  cover_url: string | null;
  start_year: number | null;
  genres: string[];
  user_rating: number | null;
  community_score: number | null;
  /** When the active reader put this series aside, null otherwise. */
  shelved_at: string | null;
};

export type RelatedSeriesDto = {
  series_id: string;
  name: string;
  book_count: number;
  books_read_count: number;
  first_book_id: string | null;
  first_book_updated_at: string | null;
  library_id: string;
  series_status: string | null;
  metadata_provider: string | null;
  cover_url: string | null;
  score: number;
  match_reasons: string[];
};

export type RecommendedSeriesDto = {
  series_id: string;
  name: string;
  library_id: string;
  series_status: string | null;
  cover_url: string | null;
  book_count: number;
  first_book_id: string | null;
  first_book_updated_at: string | null;
  metadata_provider: string | null;
  description: string | null;
  authors: string[];
  genres: string[];
  score: number;
  similarity_score: number;
  community_bonus: number;
  community_score: number | null;
  author_pts: number;
  genre_pts: number;
  reading_list_pts: number;
  publisher_pts: number;
  because_of: string[];
  match_reasons: string[];
};

export type SeriesPageDto = {
  items: SeriesDto[];
  total: number;
  page: number;
  limit: number;
};

export type CatalogShelf = "wishlist" | "in_library";
export type CatalogRating = "rated" | "unrated";
export type CatalogVolumeType = "regular" | "hs" | "oneshot" | "integral";

export function isCatalogVolumeType(value: string | undefined): value is CatalogVolumeType {
  return value === "regular" || value === "hs" || value === "oneshot" || value === "integral";
}

/** Named filters, scope and paging for the series catalog (`GET /series`). */
export type CatalogQuery = {
  /** Library UUID; maps to `library_id`. */
  libraryId?: string;
  /** Free-text search; maps to `q`. */
  search?: string;
  readingStatus?: string;
  sort?: string;
  seriesStatus?: string;
  hasMissing?: boolean;
  metadataProvider?: string;
  author?: string;
  /** Empty-series filter; maps to `no_books` or `has_books`. */
  shelf?: CatalogShelf;
  volumeType?: CatalogVolumeType;
  /** Reader rating filter; maps to `rated_only`. */
  rating?: CatalogRating;
  gap?: string;
  page?: number;
  limit?: number;
};

export function catalogQueryParams(query: CatalogQuery): URLSearchParams {
  const params = new URLSearchParams();
  if (query.libraryId) params.set("library_id", query.libraryId);
  if (query.search) params.set("q", query.search);
  if (query.readingStatus) params.set("reading_status", query.readingStatus);
  if (query.sort) params.set("sort", query.sort);
  if (query.seriesStatus) params.set("series_status", query.seriesStatus);
  if (query.hasMissing) params.set("has_missing", "true");
  if (query.metadataProvider) params.set("metadata_provider", query.metadataProvider);
  if (query.author) params.set("author", query.author);
  if (query.shelf === "wishlist") params.set("no_books", "true");
  if (query.shelf === "in_library") params.set("has_books", "true");
  if (query.volumeType) params.set("volume_type", query.volumeType);
  if (query.rating === "rated") params.set("rated_only", "true");
  if (query.rating === "unrated") params.set("rated_only", "unrated");
  if (query.gap) params.set("gap", query.gap);
  params.set("page", (query.page ?? 1).toString());
  params.set("limit", (query.limit ?? 50).toString());

  return params;
}

export async function fetchAllSeries(query: CatalogQuery = {}): Promise<SeriesPageDto> {
  return apiFetch<SeriesPageDto>(`/series?${catalogQueryParams(query).toString()}`, {
    next: { revalidate: 15, tags: [cacheTags.series()] },
  });
}

export type GapSummaryDto = {
  series_total: number;
  series_no_description: number;
  series_no_genre: number;
  series_no_authors: number;
  series_no_publishers: number;
  series_no_year: number;
  series_no_cover: number;
  series_no_community_score: number;
  books_total: number;
  books_no_summary: number;
  books_no_isbn: number;
  books_no_cover: number;
  books_no_author: number;
  books_no_publish_date: number;
  books_no_language: number;
  books_no_volume: number;
};

export async function fetchGapSummary(libraryId?: string): Promise<GapSummaryDto> {
  const params = new URLSearchParams();
  if (libraryId) params.set("library_id", libraryId);
  const qs = params.toString();

  return apiFetch<GapSummaryDto>(`/metadata/gaps/summary${qs ? `?${qs}` : ""}`, {
    next: { revalidate: 15, tags: [cacheTags.metadataGaps()] },
  });
}

export async function fetchSeriesById(seriesId: string): Promise<SeriesDto> {
  return apiFetch<SeriesDto>(`/series/${seriesId}/details`, {
    next: { revalidate: 15, tags: [cacheTags.series(), cacheTags.seriesById(seriesId)] },
  });
}

export type SeriesVolumesDto = {
  owned_volumes: number[];
  has_unnumbered_integral: boolean;
};

/**
 * Volume numbers owned across the whole series (not just the current page).
 *
 * Returns `null` when the series owns an unnumbered integral: the series is
 * complete, so nothing should be highlighted.
 */
export async function fetchSeriesOwnedVolumes(seriesId: string): Promise<number[] | null> {
  const dto = await apiFetch<SeriesVolumesDto>(`/series/${seriesId}/volumes`, {
    next: { revalidate: 15, tags: [cacheTags.series(), cacheTags.seriesById(seriesId)] },
  });
  return dto.has_unnumbered_integral ? null : dto.owned_volumes;
}

export async function fetchRelatedSeries(seriesId: string, limit = 10): Promise<RelatedSeriesDto[]> {
  return apiFetch<RelatedSeriesDto[]>(`/series/${seriesId}/related?limit=${limit}`, {
    next: { revalidate: 60, tags: [cacheTags.series(), cacheTags.seriesById(seriesId)] },
  });
}

export async function fetchSeriesRecommendations(limit = 24, sources = 5): Promise<RecommendedSeriesDto[]> {
  const params = new URLSearchParams();
  params.set("limit", limit.toString());
  params.set("sources", sources.toString());

  return apiFetch<RecommendedSeriesDto[]>(`/series/recommendations?${params.toString()}`, {
    next: { revalidate: 60, tags: [cacheTags.series()] },
  });
}

export async function fetchSeriesStatuses(): Promise<string[]> {
  return apiFetch<string[]>("/series/statuses", { next: { revalidate: 300 } });
}

export type SeriesMetadataDto = {
  series_name: string;
  authors: string[];
  genres: string[];
  description: string | null;
  publishers: string[];
  start_year: number | null;
  total_volumes: number | null;
  status: string | null;
  book_author: string | null;
  book_language: string | null;
  locked_fields: Record<string, boolean>;
};

export async function fetchSeriesMetadata(seriesId: string) {
  return apiFetch<SeriesMetadataDto>(`/series/${seriesId}/metadata`, {
    next: { revalidate: 60, tags: [cacheTags.series(), cacheTags.seriesById(seriesId)] },
  });
}

export type ProviderRatingDto = {
  provider: string;
  rating: number;
  rating_scale: number;
  rating_count: number | null;
};

export type SeriesRatingsDto = {
  user_rating: number | null;
  anilist_pulled_rating: number | null;
  provider_ratings: ProviderRatingDto[];
};

export async function fetchSeriesRatings(seriesId: string): Promise<SeriesRatingsDto> {
  return apiFetch<SeriesRatingsDto>(`/series/${seriesId}/ratings`, {
    next: { revalidate: 30, tags: [cacheTags.series(), cacheTags.seriesById(seriesId)] },
  });
}

export async function updateSeriesRating(seriesId: string, rating: number): Promise<void> {
  return apiFetch<void>(`/series/${seriesId}/rating`, {
    method: "PUT",
    body: JSON.stringify({ rating }),
  });
}

export async function deleteSeriesRating(seriesId: string): Promise<void> {
  return apiFetch<void>(`/series/${seriesId}/rating`, {
    method: "DELETE",
  });
}

export type SeriesUserActivityDto = {
  user_id: string;
  username: string;
  books_read: number;
  books_reading: number;
  books_total: number;
  last_read_at: string | null;
  rating: number | null;
};

/** Admin view: every reader's progress and rating for one series. */
export async function fetchSeriesUserActivity(seriesId: string): Promise<SeriesUserActivityDto[]> {
  return apiFetch<SeriesUserActivityDto[]>(`/admin/series/${seriesId}/activity`, {
    next: { revalidate: 15, tags: [cacheTags.series(), cacheTags.seriesById(seriesId)] },
  });
}

export async function fetchAllGenres(): Promise<string[]> {
  return apiFetch<string[]>("/series/genres").catch(() => []);
}

export type UpdateSeriesRequest = {
  new_name: string;
  authors: string[];
  author?: string | null;
  language?: string | null;
  description: string | null;
  publishers: string[];
  start_year: number | null;
  total_volumes: number | null;
  locked_fields?: Record<string, boolean>;
};

export async function updateSeries(seriesId: string, data: UpdateSeriesRequest) {
  return apiFetch<{ updated: number }>(`/series/${seriesId}`, {
    method: "PATCH",
    body: JSON.stringify(data),
  });
}

export async function deleteSeries(seriesId: string) {
  return apiFetch<void>(`/series/${seriesId}`, {
    method: "DELETE",
  });
}

export type OrphanSeriesItemDto = {
  id: string;
  name: string;
  library_id: string;
  library_name: string | null;
  metadata_provider: string | null;
  external_url: string | null;
  created_at: string;
  age_days: number;
  /** A wishlist release was detected for this series (not an in-progress download). */
  has_wishlist_release: boolean;
  /** A torrent download for this series is still downloading or importing. */
  has_active_download: boolean;
};

export type OrphanSeriesPageDto = {
  items: OrphanSeriesItemDto[];
  total: number;
  page: number;
  limit: number;
};

export type OrphanDeleteResultDto = {
  /** Series actually deleted. */
  deleted: number;
  /** Series that were already gone (`404`), treated as deleted. */
  skipped: number;
  /** Ids whose deletion failed for another reason. */
  failed: string[];
};

export async function mergeSeries(targetId: string, sourceId: string) {
  return apiFetch<{ books_moved: number; metadata_moved: number; downloads_moved: number }>(
    `/series/${targetId}/merge`,
    {
      method: "POST",
      body: JSON.stringify({ source_id: sourceId }),
    },
  );
}

export async function markSeriesRead(seriesId: string, status: "read" | "unread" = "read") {
  return apiFetch<{ updated: number }>("/series/mark-read", {
    method: "POST",
    body: JSON.stringify({ series: seriesId, status }),
  });
}
