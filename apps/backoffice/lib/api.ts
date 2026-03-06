export type LibraryDto = {
  id: string;
  name: string;
  root_path: string;
  enabled: boolean;
  book_count: number;
};

export type IndexJobDto = {
  id: string;
  library_id: string | null;
  type: string;
  status: string;
  started_at: string | null;
  finished_at: string | null;
  error_opt: string | null;
  created_at: string;
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
};

export type BookDto = {
  id: string;
  library_id: string;
  kind: string;
  title: string;
  author: string | null;
  series: string | null;
  volume: number | null;
  language: string | null;
  page_count: number | null;
  file_path: string | null;
  file_format: string | null;
  file_parse_status: string | null;
  updated_at: string;
};

export type BooksPageDto = {
  items: BookDto[];
  next_cursor: string | null;
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

export type SearchResponseDto = {
  hits: SearchHitDto[];
  estimated_total_hits: number | null;
  processing_time_ms: number | null;
};

export type SeriesDto = {
  name: string;
  book_count: number;
  first_book_id: string;
};

function config() {
  const baseUrl = process.env.API_BASE_URL || "http://api:8080";
  const token = process.env.API_BOOTSTRAP_TOKEN;
  if (!token) {
    throw new Error("API_BOOTSTRAP_TOKEN is required for backoffice");
  }
  return { baseUrl: baseUrl.replace(/\/$/, ""), token };
}

export async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const { baseUrl, token } = config();
  const headers = new Headers(init?.headers || {});
  headers.set("Authorization", `Bearer ${token}`);
  if (init?.body && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }

  const res = await fetch(`${baseUrl}${path}`, {
    ...init,
    headers,
    cache: "no-store"
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
    body: JSON.stringify({ name, root_path: rootPath })
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
    body: JSON.stringify(body)
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
    body: JSON.stringify(body)
  });
}

export async function cancelJob(id: string) {
  return apiFetch<IndexJobDto>(`/index/cancel/${id}`, { method: "POST" });
}

export async function listFolders() {
  return apiFetch<FolderItem[]>("/folders");
}

export async function listTokens() {
  return apiFetch<TokenDto[]>("/admin/tokens");
}

export async function createToken(name: string, scope: string) {
  return apiFetch<{ token: string }>("/admin/tokens", {
    method: "POST",
    body: JSON.stringify({ name, scope })
  });
}

export async function revokeToken(id: string) {
  return apiFetch<void>(`/admin/tokens/${id}`, { method: "DELETE" });
}

export async function fetchBooks(libraryId?: string, series?: string, cursor?: string, limit: number = 50): Promise<BooksPageDto> {
  const params = new URLSearchParams();
  if (libraryId) params.set("library_id", libraryId);
  if (series) params.set("series", series);
  if (cursor) params.set("cursor", cursor);
  params.set("limit", limit.toString());
  
  return apiFetch<BooksPageDto>(`/books?${params.toString()}`);
}

export async function fetchSeries(libraryId: string): Promise<SeriesDto[]> {
  return apiFetch<SeriesDto[]>(`/libraries/${libraryId}/series`);
}

export async function searchBooks(query: string, libraryId?: string, limit: number = 20): Promise<SearchResponseDto> {
  const params = new URLSearchParams();
  params.set("q", query);
  if (libraryId) params.set("library_id", libraryId);
  params.set("limit", limit.toString());
  
  return apiFetch<SearchResponseDto>(`/search?${params.toString()}`);
}

export function getBookCoverUrl(bookId: string): string {
  // Utiliser une route API locale pour éviter les problèmes CORS
  // Le navigateur ne peut pas accéder à http://api:8080 (hostname Docker interne)
  return `/api/books/${bookId}/pages/1?format=webp&width=200`;
}
