/**
 * Statistiques du tableau de bord.
 */

import { apiFetch } from "./client";

export type StatsOverview = {
  total_books: number;
  total_series: number;
  total_libraries: number;
  total_pages: number;
  total_size_bytes: number;
  total_authors: number;
};

export type ReadingStatusStats = {
  unread: number;
  reading: number;
  read: number;
};

export type FormatCount = {
  format: string;
  count: number;
};

export type LanguageCount = {
  language: string | null;
  count: number;
};

export type LibraryStatsItem = {
  library_name: string;
  book_count: number;
  size_bytes: number;
  read_count: number;
  reading_count: number;
  unread_count: number;
};

export type TopSeriesItem = {
  series: string;
  book_count: number;
  read_count: number;
  total_pages: number;
};

export type MonthlyAdditions = {
  month: string;
  books_added: number;
};

export type ProviderCount = {
  provider: string;
  count: number;
};

export type MetadataStats = {
  total_series: number;
  series_linked: number;
  series_unlinked: number;
  books_with_summary: number;
  books_with_isbn: number;
  by_provider: ProviderCount[];
};

export type CurrentlyReadingItem = {
  book_id: string;
  title: string;
  series: string | null;
  current_page: number;
  page_count: number;
  username?: string;
};

export type RecentlyReadItem = {
  book_id: string;
  title: string;
  series: string | null;
  last_read_at: string;
  username?: string;
};

export type MonthlyReading = {
  month: string;
  books_read: number;
  pages_read: number;
};

export type UserMonthlyReading = {
  month: string;
  username: string;
  books_read: number;
  pages_read: number;
};

export type JobTimePoint = {
  label: string;
  scan: number;
  rebuild: number;
  thumbnail: number;
  metadata: number;
  downloads: number;
  reading: number;
  conversion: number;
};

export type RecentDownloadItem = {
  id: string;
  series_name: string;
  status: string;
  expected_volumes: number[];
  created_at: string;
};

export type DownloadStats = {
  active_downloads: number;
  imported_downloads: number;
  error_downloads: number;
  total_downloads: number;
  available_series: number;
  total_missing_volumes: number;
  recent_downloads: RecentDownloadItem[];
};

export type StatsResponse = {
  overview: StatsOverview;
  reading_status: ReadingStatusStats;
  currently_reading: CurrentlyReadingItem[];
  recently_read: RecentlyReadItem[];
  reading_over_time: MonthlyReading[];
  users_reading_over_time: UserMonthlyReading[];
  by_format: FormatCount[];
  by_language: LanguageCount[];
  by_library: LibraryStatsItem[];
  top_series: TopSeriesItem[];
  additions_over_time: MonthlyAdditions[];
  jobs_over_time: JobTimePoint[];
  metadata: MetadataStats;
  downloads: DownloadStats;
};

export async function fetchStats(period?: "day" | "week" | "month") {
  const params = period && period !== "week" ? `?period=${period}` : "";
  return apiFetch<StatsResponse>(`/stats${params}`, {
    next: { revalidate: 30, tags: ["stats"] },
  });
}

export type StatsOverviewData = {
  overview: StatsOverview;
  reading_status: ReadingStatusStats;
  by_format: FormatCount[];
  by_language: LanguageCount[];
  metadata: MetadataStats;
  currently_reading: CurrentlyReadingItem[];
  recently_read: RecentlyReadItem[];
};

export type StatsBreakdownData = {
  by_library: LibraryStatsItem[];
  top_series: TopSeriesItem[];
  downloads: DownloadStats;
};

export async function fetchStatsOverview() {
  return apiFetch<StatsOverviewData>("/stats/overview", {
    next: { revalidate: 30, tags: ["stats"] },
  });
}

export async function fetchStatsBreakdown() {
  return apiFetch<StatsBreakdownData>("/stats/breakdown", {
    next: { revalidate: 30, tags: ["stats"] },
  });
}
