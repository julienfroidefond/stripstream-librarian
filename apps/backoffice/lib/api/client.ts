/**
 * Enveloppe HTTP bas niveau vers l'API Rust (`apiFetch`, `config`).
 */

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
