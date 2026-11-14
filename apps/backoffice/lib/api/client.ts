/**
 * Enveloppe HTTP bas niveau vers l'API Rust (`apiFetch`, `config`).
 */

import { clearActiveUser, getActiveUser, isStaleActiveUserError } from "../auth/activeUser";
import { ApiError } from "./errors";

export function config() {
  const baseUrl = process.env.API_BASE_URL || "http://api:7080";
  const token = process.env.API_BOOTSTRAP_TOKEN;
  if (!token) {
    throw new Error("API_BOOTSTRAP_TOKEN is required for backoffice");
  }
  return { baseUrl: baseUrl.replace(/\/$/, ""), token };
}

/** Extract the human-readable error from an upstream JSON (or text) body. */
function upstreamErrorMessage(text: string): string | undefined {
  try {
    const body: unknown = JSON.parse(text);
    if (body && typeof body === "object") {
      for (const key of ["error", "message"] as const) {
        const value = (body as Record<string, unknown>)[key];
        if (typeof value === "string" && value.trim()) return value;
      }
    }
  } catch {
    // Not JSON: forward the raw body below.
  }
  const trimmed = text.trim();
  return trimmed || undefined;
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
  let activeUserId: string | null = null;
  try {
    const { userId } = await getActiveUser();
    activeUserId = userId;
    if (userId) headers.set("X-As-User", userId);
  } catch {
    // Hors contexte Next.js (tests, etc.)
  }

  const { next: nextOptions, ...restInit } = init ?? {};

  // A user-scoped response must never be shared through Next's data cache: the
  // cache key only contains the URL. While impersonating a reader, always
  // bypass it even when the caller asked for `next.revalidate`.
  const cacheOptions = activeUserId
    ? { cache: "no-store" as const }
    : nextOptions
      ? { next: nextOptions }
      : { cache: "no-store" as const };

  let res = await fetch(`${baseUrl}${path}`, {
    ...restInit,
    headers,
    ...cacheOptions,
  });

  if (!res.ok) {
    let text = await res.text();
    let detail = upstreamErrorMessage(text);

    // Safety net: the selected reader no longer exists (deleted elsewhere).
    // Drop the cookie and fall back to admin mode. The API rejects the header
    // before running the handler, so a read can be replayed safely; a write is
    // not replayed (it would run with admin semantics) and still fails.
    if (activeUserId && isStaleActiveUserError(res.status, detail)) {
      await clearActiveUser();
      const method = (restInit.method ?? "GET").toUpperCase();
      if (method === "GET" || method === "HEAD") {
        const adminHeaders = new Headers(headers);
        adminHeaders.delete("X-As-User");
        res = await fetch(`${baseUrl}${path}`, {
          ...restInit,
          headers: adminHeaders,
          ...(nextOptions ? { next: nextOptions } : { cache: "no-store" as const }),
        });
        if (!res.ok) {
          text = await res.text();
          detail = upstreamErrorMessage(text);
        }
      }
    }

    if (!res.ok) {
      throw new ApiError(res.status, `API ${path} failed (${res.status}): ${text}`, detail);
    }
  }

  if (res.status === 204) {
    return null as T;
  }
  return (await res.json()) as T;
}
