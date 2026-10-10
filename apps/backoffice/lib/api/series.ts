/**
 * Séries : recherche, métadonnées, notes, genres, fusion.
 */

import { apiFetch } from "./client";

export type SeriesDto = {
  series_id: string;
  name: string;
  book_count: number;
  books_read_count: number;
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

export async function fetchAllSeries(
  libraryId?: string,
  q?: string,
  readingStatus?: string,
  page: number = 1,
  limit: number = 50,
  sort?: string,
  seriesStatus?: string,
  hasMissing?: boolean,
  metadataProvider?: string,
  author?: string,
  noBooks?: boolean,
  hasBooks?: boolean,
  volumeType?: string,
  ratedOnly?: string,
  gap?: string,
): Promise<SeriesPageDto> {
  const params = new URLSearchParams();
  if (libraryId) params.set("library_id", libraryId);
  if (q) params.set("q", q);
  if (readingStatus) params.set("reading_status", readingStatus);
  if (sort) params.set("sort", sort);
  if (seriesStatus) params.set("series_status", seriesStatus);
  if (hasMissing) params.set("has_missing", "true");
  if (metadataProvider) params.set("metadata_provider", metadataProvider);
  if (author) params.set("author", author);
  if (noBooks) params.set("no_books", "true");
  if (hasBooks) params.set("has_books", "true");
  if (volumeType) params.set("volume_type", volumeType);
  if (ratedOnly) params.set("rated_only", ratedOnly);
  if (gap) params.set("gap", gap);
  params.set("page", page.toString());
  params.set("limit", limit.toString());

  return apiFetch<SeriesPageDto>(`/series?${params.toString()}`, {
    next: { revalidate: 15, tags: ["series"] },
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
    next: { revalidate: 15, tags: ["metadata-gaps"] },
  });
}

export async function fetchSeriesById(seriesId: string): Promise<SeriesDto> {
  return apiFetch<SeriesDto>(`/series/${seriesId}/details`, {
    next: { revalidate: 15, tags: ["series", `series:${seriesId}`] },
  });
}

export async function fetchRelatedSeries(seriesId: string, limit = 10): Promise<RelatedSeriesDto[]> {
  return apiFetch<RelatedSeriesDto[]>(`/series/${seriesId}/related?limit=${limit}`, {
    next: { revalidate: 60, tags: ["series", `series:${seriesId}`] },
  });
}

export async function fetchSeriesRecommendations(limit = 24, sources = 5): Promise<RecommendedSeriesDto[]> {
  const params = new URLSearchParams();
  params.set("limit", limit.toString());
  params.set("sources", sources.toString());

  return apiFetch<RecommendedSeriesDto[]>(`/series/recommendations?${params.toString()}`, {
    next: { revalidate: 60, tags: ["series"] },
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
    next: { revalidate: 60, tags: ["series", `series:${seriesId}`] },
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
    next: { revalidate: 30, tags: ["series", `series:${seriesId}`] },
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
