/**
 * Livres : recherche, mise à jour, conversion, progression de lecture, auteurs.
 */

import { apiFetch } from "./client";
import type { IndexJobDto } from "./jobs";

export type ReadingStatus = "unread" | "reading" | "read";

export type ReadingProgressDto = {
  status: ReadingStatus;
  current_page: number | null;
  last_read_at: string | null;
};

export type BookDto = {
  id: string;
  library_id: string;
  kind: string;
  format: string | null;
  title: string;
  author: string | null;
  authors: string[];
  series: string | null;
  volume: number | null;
  volume_type: "regular" | "hs" | "oneshot" | "integral";
  language: string | null;
  page_count: number | null;
  file_path: string | null;
  file_format: string | null;
  file_parse_status: string | null;
  updated_at: string;
  reading_status: ReadingStatus;
  reading_current_page: number | null;
  reading_last_read_at: string | null;
  series_id: string | null;
  summary: string | null;
  isbn: string | null;
  publish_date: string | null;
  locked_fields?: Record<string, boolean>;
};

export type BooksPageDto = {
  items: BookDto[];
  total: number;
  page: number;
  limit: number;
};

export type SearchHitDto = {
  id: string;
  library_id: string;
  title: string;
  authors: string[];
  series: string | null;
  volume: number | null;
  volume_type: "regular" | "hs" | "oneshot" | "integral";
  kind: string;
  language: string | null;
};

export type SeriesHitDto = {
  series_id: string;
  library_id: string;
  name: string;
  book_count: number;
  books_read_count: number;
  first_book_id: string;
  first_book_updated_at: string | null;
};

export type SearchResponseDto = {
  hits: SearchHitDto[];
  series_hits: SeriesHitDto[];
  estimated_total_hits: number | null;
  processing_time_ms: number | null;
};

export async function fetchBooks(
  libraryId?: string,
  series?: string,
  page: number = 1,
  limit: number = 50,
  readingStatus?: string,
  sort?: string,
  author?: string,
  format?: string,
  metadataProvider?: string,
  q?: string,
  gap?: string,
): Promise<BooksPageDto> {
  const params = new URLSearchParams();
  if (q) params.set("q", q);
  if (libraryId) params.set("library_id", libraryId);
  if (series) params.set("series", series);
  if (readingStatus) params.set("reading_status", readingStatus);
  if (sort) params.set("sort", sort);
  if (author) params.set("author", author);
  if (format) params.set("format", format);
  if (metadataProvider) params.set("metadata_provider", metadataProvider);
  if (gap) params.set("gap", gap);
  params.set("page", page.toString());
  params.set("limit", limit.toString());

  return apiFetch<BooksPageDto>(`/books?${params.toString()}`, {
    next: { revalidate: 15, tags: ["books"] },
  });
}

export async function searchBooks(
  query: string,
  libraryId?: string,
  limit: number = 20,
): Promise<SearchResponseDto> {
  const params = new URLSearchParams();
  params.set("q", query);
  if (libraryId) params.set("library_id", libraryId);
  params.set("limit", limit.toString());

  return apiFetch<SearchResponseDto>(`/search?${params.toString()}`);
}

export function getBookCoverUrl(bookId: string, version?: string | null): string {
  const base = `/api/books/${bookId}/thumbnail`;
  return version ? `${base}?v=${encodeURIComponent(version)}` : base;
}

export async function convertBook(bookId: string) {
  return apiFetch<IndexJobDto>(`/books/${bookId}/convert`, { method: "POST" });
}

export async function fetchReadingProgress(bookId: string) {
  return apiFetch<ReadingProgressDto>(`/books/${bookId}/progress`);
}

export async function updateReadingProgress(
  bookId: string,
  status: ReadingStatus,
  currentPage?: number,
) {
  return apiFetch<ReadingProgressDto>(`/books/${bookId}/progress`, {
    method: "PATCH",
    body: JSON.stringify({ status, current_page: currentPage ?? null }),
  });
}

export type AuthorDto = {
  name: string;
  book_count: number;
  series_count: number;
};

export type AuthorsPageDto = {
  items: AuthorDto[];
  total: number;
  page: number;
  limit: number;
};

export async function fetchAuthors(
  q?: string,
  page: number = 1,
  limit: number = 20,
  sort?: string,
): Promise<AuthorsPageDto> {
  const params = new URLSearchParams();
  if (q) params.set("q", q);
  if (sort) params.set("sort", sort);
  params.set("page", page.toString());
  params.set("limit", limit.toString());

  return apiFetch<AuthorsPageDto>(`/authors?${params.toString()}`, { next: { revalidate: 30 } });
}

export type UpdateBookRequest = {
  title: string;
  author: string | null;
  authors: string[];
  series: string | null;
  volume: number | null;
  language: string | null;
  summary: string | null;
  isbn: string | null;
  publish_date: string | null;
  locked_fields?: Record<string, boolean>;
};

export async function updateBook(bookId: string, data: UpdateBookRequest) {
  return apiFetch<BookDto>(`/books/${bookId}`, {
    method: "PATCH",
    body: JSON.stringify(data),
  });
}
