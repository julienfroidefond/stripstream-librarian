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
};

export type IndexJobDto = {
  id: string;
  library_id: string | null;
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
  language: string | null;
  page_count: number | null;
  file_path: string | null;
  file_format: string | null;
  file_parse_status: string | null;
  updated_at: string;
  reading_status: ReadingStatus;
  reading_current_page: number | null;
  reading_last_read_at: string | null;
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
  author: string | null;
  series: string | null;
  volume: number | null;
  kind: string;
  language: string | null;
};

export type SeriesHitDto = {
  library_id: string;
  name: string;
  book_count: number;
  books_read_count: number;
  first_book_id: string;
};

export type SearchResponseDto = {
  hits: SearchHitDto[];
  series_hits: SeriesHitDto[];
  estimated_total_hits: number | null;
  processing_time_ms: number | null;
};

export type SeriesDto = {
  name: string;
  book_count: number;
  books_read_count: number;
  first_book_id: string;
  library_id: string;
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
  init?: RequestInit,
): Promise<T> {
  const { baseUrl, token } = config();
  const headers = new Headers(init?.headers || {});
  headers.set("Authorization", `Bearer ${token}`);
  if (init?.body && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }

  const res = await fetch(`${baseUrl}${path}`, {
    ...init,
    headers,
    cache: "no-store",
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
  return apiFetch<LibraryDto[]>("/libraries");
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
) {
  const body: {
    monitor_enabled: boolean;
    scan_mode: string;
    watcher_enabled?: boolean;
  } = {
    monitor_enabled: monitorEnabled,
    scan_mode: scanMode,
  };
  if (watcherEnabled !== undefined) {
    body.watcher_enabled = watcherEnabled;
  }
  return apiFetch<LibraryDto>(`/libraries/${libraryId}/monitoring`, {
    method: "PATCH",
    body: JSON.stringify(body),
  });
}

export async function listJobs() {
  return apiFetch<IndexJobDto[]>("/index/status");
}

export async function rebuildIndex(libraryId?: string, full?: boolean) {
  const body: { library_id?: string; full?: boolean } = {};
  if (libraryId) body.library_id = libraryId;
  if (full) body.full = true;
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

export async function createToken(name: string, scope: string) {
  return apiFetch<{ token: string }>("/admin/tokens", {
    method: "POST",
    body: JSON.stringify({ name, scope }),
  });
}

export async function revokeToken(id: string) {
  return apiFetch<void>(`/admin/tokens/${id}`, { method: "DELETE" });
}

export async function deleteToken(id: string) {
  return apiFetch<void>(`/admin/tokens/${id}/delete`, { method: "POST" });
}

export async function fetchBooks(
  libraryId?: string,
  series?: string,
  page: number = 1,
  limit: number = 50,
  readingStatus?: string,
  sort?: string,
): Promise<BooksPageDto> {
  const params = new URLSearchParams();
  if (libraryId) params.set("library_id", libraryId);
  if (series) params.set("series", series);
  if (readingStatus) params.set("reading_status", readingStatus);
  if (sort) params.set("sort", sort);
  params.set("page", page.toString());
  params.set("limit", limit.toString());

  return apiFetch<BooksPageDto>(`/books?${params.toString()}`);
}

export type SeriesPageDto = {
  items: SeriesDto[];
  total: number;
  page: number;
  limit: number;
};

export async function fetchSeries(
  libraryId: string,
  page: number = 1,
  limit: number = 50,
): Promise<SeriesPageDto> {
  const params = new URLSearchParams();
  params.set("page", page.toString());
  params.set("limit", limit.toString());

  return apiFetch<SeriesPageDto>(
    `/libraries/${libraryId}/series?${params.toString()}`,
  );
}

export async function fetchAllSeries(
  libraryId?: string,
  q?: string,
  readingStatus?: string,
  page: number = 1,
  limit: number = 50,
  sort?: string,
): Promise<SeriesPageDto> {
  const params = new URLSearchParams();
  if (libraryId) params.set("library_id", libraryId);
  if (q) params.set("q", q);
  if (readingStatus) params.set("reading_status", readingStatus);
  if (sort) params.set("sort", sort);
  params.set("page", page.toString());
  params.set("limit", limit.toString());

  return apiFetch<SeriesPageDto>(`/series?${params.toString()}`);
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

export function getBookCoverUrl(bookId: string): string {
  return `/api/books/${bookId}/thumbnail`;
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
  return apiFetch<Settings>("/settings");
}

export async function updateSetting(key: string, value: unknown) {
  return apiFetch<unknown>(`/settings/${key}`, {
    method: "POST",
    body: JSON.stringify({ value }),
  });
}

export async function getCacheStats() {
  return apiFetch<CacheStats>("/settings/cache/stats");
}

export async function clearCache() {
  return apiFetch<ClearCacheResponse>("/settings/cache/clear", {
    method: "POST",
  });
}

export async function getThumbnailStats() {
  return apiFetch<ThumbnailStats>("/settings/thumbnail/stats");
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

export type StatsResponse = {
  overview: StatsOverview;
  reading_status: ReadingStatusStats;
  by_format: FormatCount[];
  by_language: LanguageCount[];
  by_library: LibraryStatsItem[];
  top_series: TopSeriesItem[];
  additions_over_time: MonthlyAdditions[];
};

export async function fetchStats() {
  return apiFetch<StatsResponse>("/stats");
}

export type UpdateBookRequest = {
  title: string;
  author: string | null;
  authors: string[];
  series: string | null;
  volume: number | null;
  language: string | null;
};

export async function updateBook(bookId: string, data: UpdateBookRequest) {
  return apiFetch<BookDto>(`/books/${bookId}`, {
    method: "PATCH",
    body: JSON.stringify(data),
  });
}

export type SeriesMetadataDto = {
  authors: string[];
  description: string | null;
  publishers: string[];
  start_year: number | null;
  book_author: string | null;
  book_language: string | null;
};

export async function fetchSeriesMetadata(libraryId: string, seriesName: string) {
  return apiFetch<SeriesMetadataDto>(
    `/libraries/${libraryId}/series/${encodeURIComponent(seriesName)}/metadata`
  );
}

export type UpdateSeriesRequest = {
  new_name: string;
  authors: string[];
  author?: string | null;
  language?: string | null;
  description: string | null;
  publishers: string[];
  start_year: number | null;
};

export async function updateSeries(libraryId: string, seriesName: string, data: UpdateSeriesRequest) {
  return apiFetch<{ updated: number }>(`/libraries/${libraryId}/series/${encodeURIComponent(seriesName)}`, {
    method: "PATCH",
    body: JSON.stringify(data),
  });
}

export async function markSeriesRead(seriesName: string, status: "read" | "unread" = "read") {
  return apiFetch<{ updated: number }>("/series/mark-read", {
    method: "POST",
    body: JSON.stringify({ series: seriesName, status }),
  });
}
