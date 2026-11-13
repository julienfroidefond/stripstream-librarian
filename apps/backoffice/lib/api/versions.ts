/**
 * Versions des services (API Rust et indexer), pour la carte « À propos ».
 *
 * Serveur uniquement. L'indexer n'est pas derrière l'API : son URL vient d'une
 * seule variable, `INDEXER_BASE_URL` (défaut Docker `http://indexer:7081`), au
 * lieu d'une liste d'hôtes essayés tour à tour.
 */

import { apiFetch } from "./client";

const INDEXER_TIMEOUT_MS = 2000;

export function indexerBaseUrl(): string {
  return (process.env.INDEXER_BASE_URL || "http://indexer:7081").replace(/\/$/, "");
}

function versionField(body: unknown, field: string): string {
  const value =
    body && typeof body === "object" ? (body as Record<string, unknown>)[field] : undefined;
  if (typeof value !== "string" || !value) {
    throw new Error(`version response has no "${field}" field`);
  }
  return value;
}

/** Version of the Rust API (`GET /version` → `{ api }`). Rejects when unknown. */
export async function fetchApiVersion(): Promise<string> {
  return versionField(await apiFetch<unknown>("/version"), "api");
}

/** Version of the indexer (`GET {INDEXER_BASE_URL}/version` → `{ indexer }`). Rejects when unknown. */
export async function fetchIndexerVersion(): Promise<string> {
  const res = await fetch(`${indexerBaseUrl()}/version`, {
    cache: "no-store",
    signal: AbortSignal.timeout(INDEXER_TIMEOUT_MS),
  });
  if (!res.ok) throw new Error(`indexer /version failed (${res.status})`);
  return versionField(await res.json(), "indexer");
}
