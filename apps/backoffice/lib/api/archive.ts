/**
 * Séries archivées.
 */

import { apiFetch } from "./client";

export type ArchivedSeriesReadingProgressDto = {
  user_name: string;
  books_read: number;
  books_reading: number;
};

export type ArchivedSeriesItemDto = {
  id: string;
  name: string;
  library_id: string | null;
  description: string | null;
  authors: string[];
  publishers: string[];
  genres: string[];
  cover_url: string | null;
  start_year: number | null;
  total_volumes: number | null;
  status: string | null;
  book_count: number;
  archived_at: string;
  reading_progress: ArchivedSeriesReadingProgressDto[];
};

export type ArchivedBookReadingProgressDto = {
  user_name: string;
  status: string;
  current_page: number | null;
  last_read_at: string | null;
};

export type ArchivedBookItemDto = {
  id: string;
  title: string;
  kind: string;
  format: string | null;
  volume: number | null;
  volume_type: string;
  page_count: number | null;
  thumbnail_path: string | null;
  author: string | null;
  authors: string[];
  language: string | null;
  isbn: string | null;
  publish_date: string | null;
  abs_path: string | null;
  archived_at: string;
  reading_progress: ArchivedBookReadingProgressDto[];
};

export type ArchivedSeriesDetailDto = {
  id: string;
  name: string;
  library_id: string | null;
  description: string | null;
  authors: string[];
  publishers: string[];
  genres: string[];
  cover_url: string | null;
  total_volumes: number | null;
  status: string | null;
  start_year: number | null;
  archived_at: string;
  books: ArchivedBookItemDto[];
};

export async function listArchivedSeries(): Promise<ArchivedSeriesItemDto[]> {
  return apiFetch<ArchivedSeriesItemDto[]>("/admin/series/archived");
}

export async function getArchivedSeries(id: string): Promise<ArchivedSeriesDetailDto> {
  return apiFetch<ArchivedSeriesDetailDto>(`/admin/series/archived/${id}`);
}
