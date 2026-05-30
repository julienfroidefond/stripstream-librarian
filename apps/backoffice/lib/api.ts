export type LibraryDto = {
  id: string;
  name: string;
  root_path: string;
  enabled: boolean;
  book_count: number;
  monitor_enabled: boolean;
  scan_mode: string;
  next_scan_at: string | null;
  watcher_enabled: boolean;
  metadata_provider: string | null;
  fallback_metadata_provider: string | null;
  metadata_refresh_mode: string;
  next_metadata_refresh_at: string | null;
  series_count: number;
  thumbnail_book_ids: string[];
  reading_status_provider: string | null;
  reading_status_push_mode: string;
  next_reading_status_push_at: string | null;
  download_detection_mode: string;
  next_download_detection_at: string | null;
  tags: string[];
};

export type IndexJobDto = {
  id: string;
  library_id: string | null;
  library_name: string | null;
  book_id: string | null;
  type: string;
  status: string;
  started_at: string | null;
  finished_at: string | null;
  error_opt: string | null;
  created_at: string;
  stats_json: {
    scanned_files: number;
    indexed_files: number;
    removed_files: number;
    errors: number;
    warnings: number;
    refreshed?: number;
    found?: number;
    new_releases?: number;
    linked?: number;
    pushed?: number;
  } | null;
  progress_percent: number | null;
  processed_files: number | null;
  total_files: number | null;
};

export type TokenDto = {
  id: string;
  name: string;
  scope: string;
  prefix: string;
  revoked_at: string | null;
  user_id?: string;
  username?: string;
};

export type UserDto = {
  id: string;
  username: string;
  token_count: number;
  books_read: number;
  books_reading: number;
  created_at: string;
};

export type FolderItem = {
  name: string;
  path: string;
  depth: number;
  has_children: boolean;
};

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

export function config() {
  const baseUrl = process.env.API_BASE_URL || "http://api:7080";
  const token = process.env.API_BOOTSTRAP_TOKEN;
  if (!token) {
    throw new Error("API_BOOTSTRAP_TOKEN is required for backoffice");
  }
  return { baseUrl: baseUrl.replace(/\/$/, ""), token };
}

export async function apiFetch<T>(
  path: string,
  init?: RequestInit & { next?: { revalidate?: number; tags?: string[] } },
): Promise<T> {
  const { baseUrl, token } = config();
  const headers = new Headers(init?.headers || {});
  headers.set("Authorization", `Bearer ${token}`);
  if (init?.body && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }

  // Impersonation : injecte X-As-User si un user est sélectionné dans le backoffice
  try {
    const { cookies } = await import("next/headers");
    const cookieStore = await cookies();
    const asUserId = cookieStore.get("as_user_id")?.value;
    if (asUserId) headers.set("X-As-User", asUserId);
  } catch {
    // Hors contexte Next.js (tests, etc.)
  }

  const { next: nextOptions, ...restInit } = init ?? {};

  const res = await fetch(`${baseUrl}${path}`, {
    ...restInit,
    headers,
    ...(nextOptions ? { next: nextOptions } : { cache: "no-store" as const }),
  });

  if (!res.ok) {
    const text = await res.text();
    throw new Error(`API ${path} failed (${res.status}): ${text}`);
  }

  if (res.status === 204) {
    return null as T;
  }
  return (await res.json()) as T;
}

export async function fetchLibraries() {
  return apiFetch<LibraryDto[]>("/libraries", { next: { revalidate: 30 } });
}

export async function createLibrary(name: string, rootPath: string) {
  return apiFetch<LibraryDto>("/libraries", {
    method: "POST",
    body: JSON.stringify({ name, root_path: rootPath }),
  });
}

export async function deleteLibrary(id: string) {
  return apiFetch<void>(`/libraries/${id}`, { method: "DELETE" });
}

export async function scanLibrary(libraryId: string, full?: boolean) {
  const body: { full?: boolean } = {};
  if (full) body.full = true;
  return apiFetch<IndexJobDto>(`/libraries/${libraryId}/scan`, {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export async function updateLibraryMonitoring(
  libraryId: string,
  monitorEnabled: boolean,
  scanMode: string,
  watcherEnabled?: boolean,
  metadataRefreshMode?: string,
  downloadDetectionMode?: string,
) {
  const body: {
    monitor_enabled: boolean;
    scan_mode: string;
    watcher_enabled?: boolean;
    metadata_refresh_mode?: string;
    download_detection_mode?: string;
  } = {
    monitor_enabled: monitorEnabled,
    scan_mode: scanMode,
  };
  if (watcherEnabled !== undefined) {
    body.watcher_enabled = watcherEnabled;
  }
  if (metadataRefreshMode !== undefined) {
    body.metadata_refresh_mode = metadataRefreshMode;
  }
  if (downloadDetectionMode !== undefined) {
    body.download_detection_mode = downloadDetectionMode;
  }
  return apiFetch<LibraryDto>(`/libraries/${libraryId}/monitoring`, {
    method: "PATCH",
    body: JSON.stringify(body),
  });
}

export async function listJobs() {
  return apiFetch<IndexJobDto[]>("/index/status");
}

export async function rebuildIndex(libraryId?: string, full?: boolean, rescan?: boolean) {
  const body: { library_id?: string; full?: boolean; rescan?: boolean } = {};
  if (libraryId) body.library_id = libraryId;
  if (full) body.full = true;
  if (rescan) body.rescan = true;
  return apiFetch<IndexJobDto>("/index/rebuild", {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export async function rebuildThumbnails(libraryId?: string) {
  const body: { library_id?: string } = {};
  if (libraryId) body.library_id = libraryId;
  return apiFetch<IndexJobDto>("/index/thumbnails/rebuild", {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export async function regenerateThumbnails(libraryId?: string) {
  const body: { library_id?: string } = {};
  if (libraryId) body.library_id = libraryId;
  return apiFetch<IndexJobDto>("/index/thumbnails/regenerate", {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export async function cancelJob(id: string) {
  return apiFetch<IndexJobDto>(`/index/cancel/${id}`, { method: "POST" });
}

export async function listFolders(path?: string) {
  const url = path ? `/folders?path=${encodeURIComponent(path)}` : "/folders";
  return apiFetch<FolderItem[]>(url);
}

export async function listTokens() {
  return apiFetch<TokenDto[]>("/admin/tokens");
}

export async function createToken(name: string, scope: string, userId?: string) {
  return apiFetch<{ token: string }>("/admin/tokens", {
    method: "POST",
    body: JSON.stringify({ name, scope, ...(userId ? { user_id: userId } : {}) }),
  });
}

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

export async function revokeToken(id: string) {
  return apiFetch<void>(`/admin/tokens/${id}`, { method: "DELETE" });
}

export async function deleteToken(id: string) {
  return apiFetch<void>(`/admin/tokens/${id}/delete`, { method: "POST" });
}

export async function updateToken(id: string, userId: string | null) {
  return apiFetch<void>(`/admin/tokens/${id}`, {
    method: "PATCH",
    body: JSON.stringify({ user_id: userId || null }),
  });
}

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
  params.set("page", page.toString());
  params.set("limit", limit.toString());

  return apiFetch<BooksPageDto>(`/books?${params.toString()}`, {
    next: { revalidate: 15, tags: ["books"] },
  });
}

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
  params.set("page", page.toString());
  params.set("limit", limit.toString());

  return apiFetch<SeriesPageDto>(`/series?${params.toString()}`, {
    next: { revalidate: 15, tags: ["series"] },
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

export async function fetchSeriesStatuses(): Promise<string[]> {
  return apiFetch<string[]>("/series/statuses", { next: { revalidate: 300 } });
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

export type Settings = {
  image_processing: {
    format: string;
    quality: number;
    filter: string;
    max_width: number;
  };
  cache: {
    enabled: boolean;
    directory: string;
    max_size_mb: number;
  };
  limits: {
    concurrent_renders: number;
    timeout_seconds: number;
    rate_limit_per_second: number;
  };
  thumbnail: {
    enabled: boolean;
    width: number;
    height: number;
    quality: number;
    format: string;
    directory: string;
  };
};

export type CacheStats = {
  total_size_mb: number;
  file_count: number;
  directory: string;
};

export type ClearCacheResponse = {
  success: boolean;
  message: string;
};

export type ThumbnailStats = {
  total_size_mb: number;
  file_count: number;
  directory: string;
};

export async function getSettings() {
  return apiFetch<Settings>("/settings", { cache: "no-store" });
}

export async function fetchDownloadsEnabled(): Promise<boolean> {
  return apiFetch<{ enabled: boolean }>("/settings/downloads_enabled")
    .then(d => d?.enabled === true)
    .catch(() => false);
}

export async function updateSetting(key: string, value: unknown) {
  return apiFetch<unknown>(`/settings/${key}`, {
    method: "POST",
    body: JSON.stringify({ value }),
  });
}

export async function getCacheStats() {
  return apiFetch<CacheStats>("/settings/cache/stats", { cache: "no-store" });
}

export async function clearCache() {
  return apiFetch<ClearCacheResponse>("/settings/cache/clear", {
    method: "POST",
  });
}

export async function getThumbnailStats() {
  return apiFetch<ThumbnailStats>("/settings/thumbnail/stats", { cache: "no-store" });
}

// Status mappings
export type StatusMappingDto = {
  id: string;
  provider_status: string;
  mapped_status: string | null;
};

export async function fetchStatusMappings(): Promise<StatusMappingDto[]> {
  return apiFetch<StatusMappingDto[]>("/settings/status-mappings", { next: { revalidate: 60 } });
}

export async function upsertStatusMapping(provider_status: string, mapped_status: string): Promise<StatusMappingDto> {
  return apiFetch<StatusMappingDto>("/settings/status-mappings", {
    method: "POST",
    body: JSON.stringify({ provider_status, mapped_status }),
  });
}

export async function deleteStatusMapping(id: string): Promise<void> {
  await apiFetch<unknown>(`/settings/status-mappings/${id}`, { method: "DELETE" });
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

// ---------------------------------------------------------------------------
// Authors
// ---------------------------------------------------------------------------

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
  return apiFetch<SeriesMetadataDto>(
    `/series/${seriesId}/metadata`
  );
}

export async function fetchAllGenres(): Promise<string[]> {
  return fetch("/api/series/genres").then((r) => r.json()).catch(() => []);
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

export type KomgaSyncRequest = {
  url: string;
  username: string;
  password: string;
  user_id: string;
};

export type KomgaSyncResponse = {
  id: string;
  komga_url: string;
  user_id?: string;
  total_komga_read: number;
  matched: number;
  already_read: number;
  newly_marked: number;
  matched_books: string[];
  newly_marked_books: string[];
  unmatched: string[];
  created_at: string;
};

export type KomgaSyncReportSummary = {
  id: string;
  komga_url: string;
  user_id?: string;
  total_komga_read: number;
  matched: number;
  already_read: number;
  newly_marked: number;
  unmatched_count: number;
  created_at: string;
};

export async function syncKomga(req: KomgaSyncRequest) {
  return apiFetch<KomgaSyncResponse>("/komga/sync", {
    method: "POST",
    body: JSON.stringify(req),
  });
}

export async function listKomgaReports() {
  return apiFetch<KomgaSyncReportSummary[]>("/komga/reports");
}

export async function getKomgaReport(id: string) {
  return apiFetch<KomgaSyncResponse>(`/komga/reports/${id}`);
}

// ---------------------------------------------------------------------------
// External Metadata
// ---------------------------------------------------------------------------

export type SeriesCandidateDto = {
  provider: string;
  external_id: string;
  title: string;
  authors: string[];
  description: string | null;
  publishers: string[];
  start_year: number | null;
  total_volumes: number | null;
  cover_url: string | null;
  external_url: string | null;
  confidence: number;
  metadata_json: Record<string, unknown>;
};

export type ExternalMetadataLinkDto = {
  id: string;
  library_id: string;
  series_name: string;
  provider: string;
  external_id: string;
  external_url: string | null;
  status: string;
  confidence: number | null;
  metadata_json: Record<string, unknown>;
  total_volumes_external: number | null;
  matched_at: string;
  approved_at: string | null;
  synced_at: string | null;
};

export type FieldChange = {
  field: string;
  old_value?: unknown;
  new_value?: unknown;
};

export type SeriesSyncReport = {
  fields_updated: FieldChange[];
  fields_skipped: FieldChange[];
};

export type BookSyncReport = {
  book_id: string;
  title: string;
  volume: number | null;
  fields_updated: FieldChange[];
  fields_skipped: FieldChange[];
};

export type SyncReport = {
  series: SeriesSyncReport | null;
  books: BookSyncReport[];
  books_matched: number;
  books_unmatched: number;
  books_message?: string;
};

export type MissingBooksDto = {
  total_external: number;
  total_local: number;
  missing_count: number;
  missing_books: {
    title: string | null;
    volume_number: number | null;
    external_book_id: string | null;
    cover_url: string | null;
  }[];
};

export async function searchMetadata(libraryId: string, seriesName: string, provider?: string) {
  return apiFetch<SeriesCandidateDto[]>("/metadata/search", {
    method: "POST",
    body: JSON.stringify({ library_id: libraryId, series_name: seriesName, provider: provider || undefined }),
  });
}

export async function createMetadataMatch(data: {
  library_id: string;
  series_name: string;
  provider: string;
  external_id: string;
  external_url?: string | null;
  confidence?: number | null;
  title: string;
  metadata_json: Record<string, unknown>;
  total_volumes?: number | null;
}) {
  return apiFetch<ExternalMetadataLinkDto>("/metadata/match", {
    method: "POST",
    body: JSON.stringify(data),
  });
}

export async function approveMetadataMatch(id: string, syncSeries: boolean, syncBooks: boolean) {
  return apiFetch<{ status: string; report: SyncReport }>(`/metadata/approve/${id}`, {
    method: "POST",
    body: JSON.stringify({ sync_series: syncSeries, sync_books: syncBooks }),
  });
}

export async function rejectMetadataMatch(id: string) {
  return apiFetch<{ status: string }>(`/metadata/reject/${id}`, {
    method: "POST",
  });
}

export async function getMetadataLink(seriesId: string) {
  const params = new URLSearchParams();
  params.set("series_id", seriesId);
  return apiFetch<ExternalMetadataLinkDto[]>(`/metadata/links?${params.toString()}`, {
    next: { revalidate: 10, tags: ["metadata", `series:${seriesId}`] },
  });
}

export async function getReadingStatusLink(seriesId: string) {
  return apiFetch<AnilistSeriesLinkDto>(
    `/series/${seriesId}/anilist`
  );
}

export async function getMissingBooks(linkId: string) {
  return apiFetch<MissingBooksDto>(`/metadata/missing/${linkId}`, {
    next: { revalidate: 10, tags: ["metadata", `link:${linkId}`] },
  });
}

export async function deleteMetadataLink(id: string) {
  return apiFetch<{ deleted: boolean }>(`/metadata/links/${id}`, {
    method: "DELETE",
  });
}

export async function updateLibraryMetadataProvider(libraryId: string, provider: string | null, fallbackProvider?: string | null) {
  return apiFetch<LibraryDto>(`/libraries/${libraryId}/metadata-provider`, {
    method: "PATCH",
    body: JSON.stringify({ metadata_provider: provider, fallback_metadata_provider: fallbackProvider }),
  });
}

// ---------------------------------------------------------------------------
// Batch Metadata
// ---------------------------------------------------------------------------

export type MetadataBatchReportDto = {
  job_id: string;
  status: string;
  total_series: number;
  processed: number;
  auto_matched: number;
  no_results: number;
  too_many_results: number;
  low_confidence: number;
  already_linked: number;
  errors: number;
};

export type MetadataBatchResultDto = {
  id: string;
  series_id?: string;
  series_name: string;
  status: string;
  provider_used: string | null;
  fallback_used: boolean;
  candidates_count: number;
  best_confidence: number | null;
  best_candidate_json: Record<string, unknown> | null;
  link_id: string | null;
  error_message: string | null;
};

export async function startMetadataBatch(libraryId?: string, forceRematch = false) {
  const payload: Record<string, unknown> = {};
  if (libraryId) payload.library_id = libraryId;
  if (forceRematch) payload.force_rematch = true;
  return apiFetch<{ id: string | null; status: string }>("/metadata/batch", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}

export async function startMetadataRefresh(libraryId?: string) {
  return apiFetch<{ id: string | null; status: string }>("/metadata/refresh", {
    method: "POST",
    body: JSON.stringify(libraryId ? { library_id: libraryId } : {}),
  });
}

export async function startMetadataRefreshAll(libraryId?: string) {
  return apiFetch<{ id: string | null; status: string }>("/metadata/refresh-all", {
    method: "POST",
    body: JSON.stringify(libraryId ? { library_id: libraryId } : {}),
  });
}

export async function startReadingStatusMatch(libraryId?: string) {
  return apiFetch<{ id: string | null; status: string }>("/reading-status/match", {
    method: "POST",
    body: JSON.stringify(libraryId ? { library_id: libraryId } : {}),
  });
}

export type ReadingStatusMatchReportDto = {
  job_id: string;
  status: string;
  total_series: number;
  linked: number;
  already_linked: number;
  no_results: number;
  ambiguous: number;
  errors: number;
};

export type ReadingStatusMatchResultDto = {
  id: string;
  series_id?: string;
  series_name: string;
  status: "linked" | "already_linked" | "no_results" | "ambiguous" | "error";
  anilist_id: number | null;
  anilist_title: string | null;
  anilist_url: string | null;
  error_message: string | null;
};

export async function getReadingStatusMatchReport(jobId: string) {
  return apiFetch<ReadingStatusMatchReportDto>(`/reading-status/match/${jobId}/report`);
}

export async function getReadingStatusMatchResults(jobId: string) {
  return apiFetch<ReadingStatusMatchResultDto[]>(`/reading-status/match/${jobId}/results`);
}

export async function startReadingStatusPush(libraryId?: string) {
  return apiFetch<{ id: string | null; status: string }>("/reading-status/push", {
    method: "POST",
    body: JSON.stringify(libraryId ? { library_id: libraryId } : {}),
  });
}

export type ReadingStatusPushReportDto = {
  job_id: string;
  status: string;
  total_series: number;
  pushed: number;
  skipped: number;
  no_books: number;
  errors: number;
};

export type ReadingStatusPushResultDto = {
  id: string;
  series_id?: string;
  series_name: string;
  status: "pushed" | "skipped" | "no_books" | "error";
  anilist_id: number | null;
  anilist_title: string | null;
  anilist_url: string | null;
  anilist_status: string | null;
  progress_volumes: number | null;
  error_message: string | null;
};

export async function getReadingStatusPushReport(jobId: string) {
  return apiFetch<ReadingStatusPushReportDto>(`/reading-status/push/${jobId}/report`);
}

export async function getReadingStatusPushResults(jobId: string) {
  return apiFetch<ReadingStatusPushResultDto[]>(`/reading-status/push/${jobId}/results`);
}

export async function startDownloadDetection(libraryId?: string) {
  return apiFetch<{ id: string | null; status: string }>("/download-detection/start", {
    method: "POST",
    body: JSON.stringify(libraryId ? { library_id: libraryId } : {}),
  });
}

export type AvailableReleaseDto = {
  title: string;
  size: number;
  download_url: string | null;
  indexer: string | null;
  seeders: number | null;
  matched_missing_volumes: number[];
  all_volumes: number[];
  has_failed?: boolean;
  detected_at?: string;
};

export type DownloadDetectionReportDto = {
  job_id: string;
  status: string;
  total_series: number;
  found: number;
  new_releases?: number;
  not_found: number;
  no_missing: number;
  no_metadata: number;
  errors: number;
};

export type DownloadDetectionResultDto = {
  id: string;
  series_id?: string;
  series_name: string;
  status: "found" | "not_found" | "no_missing" | "no_metadata" | "error";
  missing_count: number;
  available_releases: AvailableReleaseDto[] | null;
  error_message: string | null;
};

export type AvailableDownloadDto = {
  id: string;
  series_id: string;
  series_name: string;
  missing_count: number;
  available_releases: AvailableReleaseDto[] | null;
  updated_at: string;
  failed_download_count: number;
};

export type LatestFoundPerLibraryDto = {
  library_id: string;
  library_name: string;
  results: AvailableDownloadDto[];
};

export async function getDownloadDetectionReport(jobId: string) {
  return apiFetch<DownloadDetectionReportDto>(`/download-detection/${jobId}/report`);
}

export async function getDownloadDetectionResults(jobId: string, status?: string) {
  const url = status
    ? `/download-detection/${jobId}/results?status=${encodeURIComponent(status)}`
    : `/download-detection/${jobId}/results`;
  return apiFetch<DownloadDetectionResultDto[]>(url);
}

export type RefreshFieldDiff = {
  field: string;
  old?: unknown;
  new?: unknown;
};

export type RefreshBookDiff = {
  book_id: string;
  title: string;
  volume: number | null;
  changes: RefreshFieldDiff[];
};

export type RefreshSeriesResult = {
  series_id?: string;
  series_name: string;
  provider: string;
  status: string; // "updated" | "unchanged" | "error"
  series_changes: RefreshFieldDiff[];
  book_changes: RefreshBookDiff[];
  error?: string;
};

export type MetadataRefreshReportDto = {
  job_id: string;
  status: string;
  total_links: number;
  refreshed: number;
  unchanged: number;
  errors: number;
  changes: RefreshSeriesResult[];
};

export async function getMetadataRefreshReport(jobId: string) {
  return apiFetch<MetadataRefreshReportDto>(`/metadata/refresh/${jobId}/report`);
}

export async function getMetadataBatchReport(jobId: string) {
  return apiFetch<MetadataBatchReportDto>(`/metadata/batch/${jobId}/report`);
}

export async function getMetadataBatchResults(jobId: string, status?: string) {
  const params = status ? `?status=${status}` : "";
  return apiFetch<MetadataBatchResultDto[]>(`/metadata/batch/${jobId}/results${params}`);
}

// ---------------------------------------------------------------------------
// Prowlarr
// ---------------------------------------------------------------------------

export type ProwlarrCategory = {
  id: number;
  name: string | null;
};

export type ProwlarrRelease = {
  guid: string;
  title: string;
  size: number;
  downloadUrl: string | null;
  indexer: string | null;
  seeders: number | null;
  leechers: number | null;
  publishDate: string | null;
  protocol: string | null;
  infoUrl: string | null;
  categories: ProwlarrCategory[] | null;
  matchedMissingVolumes: number[] | null;
  allVolumes?: number[];
};

export type ProwlarrSearchResponse = {
  results: ProwlarrRelease[];
  query: string;
};

export type ProwlarrTestResponse = {
  success: boolean;
  message: string;
  indexer_count: number | null;
};

// ---------------------------------------------------------------------------
// qBittorrent
// ---------------------------------------------------------------------------

export type QBittorrentAddResponse = {
  success: boolean;
  message: string;
  torrent_download_id?: string | null;
};

export type TorrentDownloadDto = {
  id: string;
  library_id: string;
  series_id?: string;
  series_name: string;
  expected_volumes: number[];
  qb_hash: string | null;
  content_path: string | null;
  status: "downloading" | "completed" | "importing" | "imported" | "partial" | "no_files_imported" | "error";
  imported_files: Array<{ volume: number; source: string; destination: string; already_existed?: boolean }> | null;
  error_message: string | null;
  progress: number;
  download_speed: number;
  eta: number;
  created_at: string;
  updated_at: string;
};

export async function fetchTorrentDownloads(): Promise<TorrentDownloadDto[]> {
  return apiFetch<TorrentDownloadDto[]>("/torrent-downloads");
}

export type QBittorrentTestResponse = {
  success: boolean;
  message: string;
  version: string | null;
};

// ---------------------------------------------------------------------------
// Reading lists
// ---------------------------------------------------------------------------

export type ReadingListDto = {
  id: string;
  name: string;
  description: string | null;
  series_count: number;
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
  return apiFetch<ReadingListDto[]>("/reading-lists");
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
