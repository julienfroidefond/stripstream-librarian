/**
 * Types AniList.
 */

export type AnilistStatusDto = {
  connected: boolean;
  user_id: number;
  username: string;
  site_url: string;
};

export type AnilistMediaResultDto = {
  id: number;
  title_romaji: string | null;
  title_english: string | null;
  title_native: string | null;
  site_url: string;
  status: string | null;
  volumes: number | null;
};

export type AnilistSeriesLinkDto = {
  library_id: string;
  series_name: string;
  anilist_id: number;
  anilist_title: string | null;
  anilist_url: string | null;
  status: string;
  linked_at: string;
  synced_at: string | null;
};

export type AnilistUnlinkedSeriesDto = {
  library_id: string;
  library_name: string;
  series_name: string;
};

export type AnilistSyncPreviewItemDto = {
  series_name: string;
  anilist_id: number;
  anilist_title: string | null;
  anilist_url: string | null;
  status: "PLANNING" | "CURRENT" | "COMPLETED";
  progress_volumes: number;
  books_read: number;
  book_count: number;
};

export type AnilistSyncItemDto = {
  series_name: string;
  anilist_title: string | null;
  anilist_url: string | null;
  status: string;
  progress_volumes: number;
};

export type AnilistSyncReportDto = {
  synced: number;
  skipped: number;
  errors: string[];
  items: AnilistSyncItemDto[];
};

export type AnilistPullItemDto = {
  series_name: string;
  anilist_title: string | null;
  anilist_url: string | null;
  anilist_status: string;
  books_updated: number;
};

export type AnilistPullReportDto = {
  updated: number;
  skipped: number;
  errors: string[];
  items: AnilistPullItemDto[];
};
