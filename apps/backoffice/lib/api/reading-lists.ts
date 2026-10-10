/**
 * Listes de lecture.
 */

import { apiFetch } from "./client";

export type ReadingListDto = {
  id: string;
  name: string;
  description: string | null;
  series_count: number;
  book_count: number;
  books_read_count: number;
  preview_covers: string[];
  created_at: string;
  updated_at: string;
};

export type ReadingListSeriesDto = {
  id: string;
  name: string;
  cover_url: string | null;
  first_book_id: string | null;
  first_book_updated_at: string | null;
  provider: string | null;
  external_id: string | null;
  external_url: string | null;
  library_id: string;
  library_name: string;
  position: number;
};

export type ReadingListDetailDto = {
  id: string;
  name: string;
  description: string | null;
  items: ReadingListSeriesDto[];
  created_at: string;
  updated_at: string;
};

export type SeriesMembershipDto = {
  series_id: string;
  list_id: string;
};

export async function fetchReadingLists(): Promise<ReadingListDto[]> {
  return apiFetch<ReadingListDto[]>("/reading-lists", {
    next: { revalidate: 30 },
  });
}

export async function fetchSeriesReadingLists(seriesId: string): Promise<ReadingListDto[]> {
  return apiFetch<ReadingListDto[]>(`/reading-lists?series_id=${seriesId}`, {
    next: { revalidate: 15, tags: [`series:${seriesId}`, "reading-lists"] },
  });
}

export async function fetchSeriesMemberships(): Promise<SeriesMembershipDto[]> {
  return apiFetch<SeriesMembershipDto[]>("/reading-lists/memberships");
}

export async function fetchReadingList(id: string): Promise<ReadingListDetailDto> {
  return apiFetch<ReadingListDetailDto>(`/reading-lists/${id}`);
}

export async function createReadingList(
  name: string,
  description?: string
): Promise<ReadingListDto> {
  return apiFetch<ReadingListDto>("/reading-lists", {
    method: "POST",
    body: JSON.stringify({ name, description }),
  });
}

export async function updateReadingList(
  id: string,
  data: { name?: string; description?: string }
): Promise<ReadingListDto> {
  return apiFetch<ReadingListDto>(`/reading-lists/${id}`, {
    method: "PATCH",
    body: JSON.stringify(data),
  });
}

export async function deleteReadingList(id: string): Promise<void> {
  await apiFetch(`/reading-lists/${id}`, { method: "DELETE" });
}

export async function addSeriesToReadingList(
  listId: string,
  seriesId: string
): Promise<void> {
  await apiFetch(`/reading-lists/${listId}/series`, {
    method: "POST",
    body: JSON.stringify({ series_id: seriesId }),
  });
}

export async function removeSeriesFromReadingList(
  listId: string,
  seriesId: string
): Promise<void> {
  await apiFetch(`/reading-lists/${listId}/series/${seriesId}`, { method: "DELETE" });
}

export async function reorderReadingListSeries(
  listId: string,
  seriesIds: string[]
): Promise<void> {
  await apiFetch(`/reading-lists/${listId}/series/reorder`, {
    method: "PUT",
    body: JSON.stringify({ series_ids: seriesIds }),
  });
}
