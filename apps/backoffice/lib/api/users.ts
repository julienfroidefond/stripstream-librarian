/**
 * Utilisateurs, restrictions de genres et vue d'ensemble de lecture.
 */

import { apiFetch } from "./client";
import type { ReadingStatus } from "./books";

export type UserDto = {
  id: string;
  username: string;
  token_count: number;
  books_read: number;
  books_reading: number;
  created_at: string;
};

export async function fetchUsers(): Promise<UserDto[]> {
  return apiFetch<UserDto[]>("/admin/users", { next: { revalidate: 60 } });
}

export async function createUser(username: string): Promise<UserDto> {
  return apiFetch<UserDto>("/admin/users", {
    method: "POST",
    body: JSON.stringify({ username }),
  });
}

export async function deleteUser(id: string): Promise<void> {
  return apiFetch<void>(`/admin/users/${id}`, { method: "DELETE" });
}

export async function updateUser(id: string, username: string): Promise<void> {
  return apiFetch<void>(`/admin/users/${id}`, {
    method: "PATCH",
    body: JSON.stringify({ username }),
  });
}

export type UserGenreRestrictionsDto = {
  blocked_genres: string[];
};

export async function fetchUserGenreRestrictions(userId: string): Promise<UserGenreRestrictionsDto> {
  return apiFetch<UserGenreRestrictionsDto>(`/admin/users/${userId}/genre-restrictions`);
}

export async function setUserGenreRestrictions(userId: string, blockedGenres: string[]): Promise<UserGenreRestrictionsDto> {
  return apiFetch<UserGenreRestrictionsDto>(`/admin/users/${userId}/genre-restrictions`, {
    method: "PUT",
    body: JSON.stringify({ blocked_genres: blockedGenres }),
  });
}

export type UserReadingOverviewItemDto = {
  book_id: string;
  title: string;
  series: string | null;
  series_id: string | null;
  current_page: number;
  page_count: number;
  last_read_at: string | null;
};

export type UserReadingOverviewSeriesBookDto = {
  book_id: string;
  title: string;
  volume: number | null;
  volume_type: "regular" | "hs" | "oneshot" | "integral";
  status: ReadingStatus;
  current_page: number;
  page_count: number;
  last_read_at: string | null;
};

export type UserReadingOverviewSeriesDto = {
  series_id: string | null;
  series_name: string;
  books_total: number;
  books_read: number;
  books_reading: number;
  books_unread: number;
  last_read_at: string | null;
  books: UserReadingOverviewSeriesBookDto[];
};

export type UserReadingOverviewDto = {
  user_id: string;
  username: string;
  books_read: number;
  books_reading: number;
  series_in_progress: number;
  last_read_at: string | null;
  currently_reading: UserReadingOverviewItemDto[];
  recently_read: UserReadingOverviewItemDto[];
  series_progress: UserReadingOverviewSeriesDto[];
};

export async function getReadingOverview(): Promise<UserReadingOverviewDto[]> {
  return apiFetch<UserReadingOverviewDto[]>("/admin/reading-overview");
}
