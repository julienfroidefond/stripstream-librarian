import { loadEnvLocal } from "../env";

/**
 * Direct calls to the Rust API for E2E fixtures and cleanup.
 *
 * The backoffice has no admin proxy routes for users, tokens or reading lists,
 * so specs use the bootstrap token from `.env.local` to prepare/remove data
 * without going through the UI under test.
 */

export type ReadingStatus = "unread" | "reading" | "read";
export type ReadingListSummary = { id: string; name: string };
export type TokenSummary = { id: string; name: string; revoked_at?: string | null };
export type UserSummary = { id: string; username: string };
export type LibrarySummary = {
  id: string;
  name: string;
  root_path: string;
  book_count?: number;
  series_count?: number;
};
export type SeriesSummary = { series_id: string; name: string; library_id: string };
export type SeriesMetadataSummary = {
  series_name: string;
  authors: string[];
  genres: string[];
  description: string | null;
};
export type MetadataLinkSummary = {
  id: string;
  series_name: string;
  provider: string;
  status: string;
  is_primary: boolean;
};
export type ArchivedSeriesSummary = {
  id: string;
  name: string;
  library_id?: string | null;
};
export type IndexJobSummary = {
  id: string;
  library_id: string | null;
  type: string;
  status: string;
  error_opt?: string | null;
};
export type BookSummary = {
  id: string;
  title: string;
  reading_status: ReadingStatus;
  reading_current_page: number | null;
};
export type BooksPageSummary = {
  items: BookSummary[];
  total: number;
  page: number;
  limit: number;
};
export type ReadingProgressSummary = {
  status: ReadingStatus;
  current_page: number | null;
  last_read_at: string | null;
};

/** Erreur HTTP de l'API, avec le statut pour permettre des replis ciblés. */
export class ApiRequestError extends Error {
  constructor(
    method: string,
    path: string,
    public readonly status: number,
    public readonly body: string
  ) {
    super(`${method} ${path} a échoué (HTTP ${status}) : ${body}`);
    this.name = "ApiRequestError";
  }
}

function resolveEnv(): { baseUrl: string; token: string } {
  const env = loadEnvLocal();
  const baseUrl = process.env.API_BASE_URL || env.API_BASE_URL || "http://localhost:7080";
  const token = process.env.API_BOOTSTRAP_TOKEN || env.API_BOOTSTRAP_TOKEN;
  if (!token) {
    throw new Error(
      "API_BOOTSTRAP_TOKEN introuvable. Ajoute-le à apps/backoffice/.env.local ou passe-le en variable d'environnement."
    );
  }
  return { baseUrl: baseUrl.replace(/\/$/, ""), token };
}

export function apiBaseUrl(): string {
  return resolveEnv().baseUrl;
}

export function bootstrapToken(): string {
  return resolveEnv().token;
}

export async function apiRequest<T>(
  path: string,
  options: { method?: string; body?: unknown; headers?: Record<string, string> } = {}
): Promise<T> {
  const method = options.method ?? "GET";
  const res = await fetch(`${apiBaseUrl()}${path}`, {
    method,
    headers: {
      Authorization: `Bearer ${bootstrapToken()}`,
      ...(options.body === undefined ? {} : { "Content-Type": "application/json" }),
      ...options.headers,
    },
    body: options.body === undefined ? undefined : JSON.stringify(options.body),
  });

  if (!res.ok) {
    throw new ApiRequestError(method, path, res.status, await res.text());
  }
  if (res.status === 204) return undefined as T;

  const text = await res.text();
  return (text ? JSON.parse(text) : undefined) as T;
}

export async function isDownloadsEnabled(): Promise<boolean> {
  const payload = await apiRequest<{ enabled?: boolean }>("/settings/downloads_enabled");
  return payload.enabled === true;
}

export async function createReadingList(
  name: string,
  description?: string
): Promise<ReadingListSummary> {
  return apiRequest<ReadingListSummary>("/reading-lists", {
    method: "POST",
    body: { name, description },
  });
}

export async function deleteReadingList(id: string): Promise<void> {
  await apiRequest<void>(`/reading-lists/${id}`, { method: "DELETE" });
}

/** Remove every reading list carrying this exact name (test leftovers). */
export async function deleteReadingListsByName(name: string): Promise<void> {
  const lists = await apiRequest<ReadingListSummary[]>("/reading-lists");
  await Promise.all(
    lists
      .filter((list) => list.name === name)
      .map((list) => deleteReadingList(list.id).catch(() => undefined))
  );
}

/** Revoke then hard-delete every token carrying this exact name. */
export async function deleteTokensByName(name: string): Promise<void> {
  const tokens = await apiRequest<TokenSummary[]>("/admin/tokens");
  for (const token of tokens.filter((item) => item.name === name)) {
    await apiRequest(`/admin/tokens/${token.id}`, { method: "DELETE" }).catch(() => undefined);
    await apiRequest(`/admin/tokens/${token.id}/delete`, { method: "POST" }).catch(() => undefined);
  }
}

/** Remove every user carrying this exact username. */
export async function deleteUsersByUsername(username: string): Promise<void> {
  const users = await apiRequest<UserSummary[]>("/admin/users");
  await Promise.all(
    users
      .filter((user) => user.username === username)
      .map((user) =>
        apiRequest(`/admin/users/${user.id}`, { method: "DELETE" }).catch(() => undefined)
      )
  );
}

/** Exact username lookup; the admin users endpoint returns every account. */
export async function findUserByUsername(username: string): Promise<UserSummary | null> {
  const users = await apiRequest<UserSummary[]>("/admin/users");
  return users.find((user) => user.username === username) ?? null;
}

// ─── Bibliothèques ───────────────────────────────────────────────────────────

export async function listLibraries(): Promise<LibrarySummary[]> {
  return apiRequest<LibrarySummary[]>("/libraries");
}

export async function createLibrary(name: string, rootPath: string): Promise<LibrarySummary> {
  return apiRequest<LibrarySummary>("/libraries", {
    method: "POST",
    body: { name, root_path: rootPath },
  });
}

export async function deleteLibrary(id: string): Promise<void> {
  await apiRequest<void>(`/libraries/${id}`, { method: "DELETE" });
}

// ─── Séries ──────────────────────────────────────────────────────────────────

export async function createSeries(libraryId: string, name: string): Promise<SeriesSummary> {
  return apiRequest<SeriesSummary>("/series/create", {
    method: "POST",
    body: { library_id: libraryId, name },
  });
}

export async function updateSeries(
  id: string,
  body: {
    new_name: string;
    authors?: string[];
    genres?: string[];
  }
): Promise<void> {
  await apiRequest<void>(`/series/${id}`, { method: "PATCH", body });
}

export async function deleteSeries(id: string): Promise<void> {
  await apiRequest<void>(`/series/${id}`, { method: "DELETE" });
}

export async function listArchivedSeries(): Promise<ArchivedSeriesSummary[]> {
  return apiRequest<ArchivedSeriesSummary[]>("/admin/series/archived");
}

// ─── Métadonnées ─────────────────────────────────────────────────────────────

export async function getSeriesMetadata(id: string): Promise<SeriesMetadataSummary> {
  return apiRequest<SeriesMetadataSummary>(`/series/${id}/metadata`);
}

export async function createMetadataMatch(body: {
  library_id: string;
  series_name: string;
  provider: string;
  external_id: string;
  title: string;
  metadata_json: Record<string, unknown>;
  external_url?: string;
  confidence?: number;
  total_volumes?: number;
}): Promise<MetadataLinkSummary> {
  return apiRequest<MetadataLinkSummary>("/metadata/match", { method: "POST", body });
}

export async function approveMetadataLink(
  id: string,
  options: { syncSeries?: boolean; syncBooks?: boolean } = {}
): Promise<void> {
  await apiRequest<void>(`/metadata/approve/${id}`, {
    method: "POST",
    body: {
      sync_series: options.syncSeries ?? false,
      sync_books: options.syncBooks ?? false,
    },
  });
}

export async function deleteMetadataLink(id: string): Promise<void> {
  await apiRequest<void>(`/metadata/links/${id}`, { method: "DELETE" });
}

// ─── Jobs ────────────────────────────────────────────────────────────────────

export async function getIndexJob(id: string): Promise<IndexJobSummary> {
  return apiRequest<IndexJobSummary>(`/index/jobs/${id}`);
}

// ─── Lecture ─────────────────────────────────────────────────────────────────

/** List books as seen by a reader (X-As-User), default title order. */
export async function listBooksForUser(
  userId: string,
  options: { limit?: number; sort?: "title" | "latest" } = {}
): Promise<BooksPageSummary> {
  const params = new URLSearchParams({ limit: String(options.limit ?? 50) });
  if (options.sort) params.set("sort", options.sort);
  return apiRequest<BooksPageSummary>(`/books?${params.toString()}`, {
    headers: { "X-As-User": userId },
  });
}

export async function getBookReadingProgress(
  bookId: string,
  userId: string
): Promise<ReadingProgressSummary> {
  return apiRequest<ReadingProgressSummary>(`/books/${bookId}/progress`, {
    headers: { "X-As-User": userId },
  });
}

export async function updateBookReadingProgress(
  bookId: string,
  userId: string,
  status: ReadingStatus,
  currentPage?: number
): Promise<ReadingProgressSummary> {
  const body: { status: ReadingStatus; current_page?: number } = { status };
  if (currentPage !== undefined) body.current_page = currentPage;
  return apiRequest<ReadingProgressSummary>(`/books/${bookId}/progress`, {
    method: "PATCH",
    headers: { "X-As-User": userId },
    body,
  });
}
