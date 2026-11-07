import { loadEnvLocal } from "../env";

/**
 * Direct calls to the Rust API for E2E fixtures and cleanup.
 *
 * The backoffice has no admin proxy routes for users, tokens or reading lists,
 * so specs use the bootstrap token from `.env.local` to prepare/remove data
 * without going through the UI under test.
 */

export type ReadingListSummary = { id: string; name: string };
export type TokenSummary = { id: string; name: string; revoked_at?: string | null };
export type UserSummary = { id: string; username: string };

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
  options: { method?: string; body?: unknown } = {}
): Promise<T> {
  const method = options.method ?? "GET";
  const res = await fetch(`${apiBaseUrl()}${path}`, {
    method,
    headers: {
      Authorization: `Bearer ${bootstrapToken()}`,
      ...(options.body === undefined ? {} : { "Content-Type": "application/json" }),
    },
    body: options.body === undefined ? undefined : JSON.stringify(options.body),
  });

  if (!res.ok) {
    throw new Error(`${method} ${path} a échoué (HTTP ${res.status}) : ${await res.text()}`);
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
