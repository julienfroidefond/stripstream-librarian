/**
 * The single browser transport for the Next.js proxy routes.
 *
 * Client Components call same-origin `/api/...` routes; those routes own the
 * session and forward to the Rust API. This module owns the parts every caller
 * used to repeat: method/body/headers, JSON parsing, status checks and the
 * `{ error }` envelope.
 *
 * Domain functions (`browserApi.series.update`, ...) sit on top and keep URLs
 * out of the views. The adapter is deliberately framework-free and only reads
 * the global `fetch`, so component tests can keep stubbing one function.
 */

import { ApiError } from "../errors";

export type BrowserMethod = "GET" | "POST" | "PUT" | "PATCH" | "DELETE";

export interface BrowserRequestOptions {
  method?: BrowserMethod;
  /** A plain object is JSON-encoded; `BodyInit` values are sent as-is. */
  body?: unknown;
  headers?: HeadersInit;
  signal?: AbortSignal;
  cache?: RequestCache;
}

/** Details extracted from a non-2xx proxy response. */
async function errorDetail(response: Response): Promise<string | undefined> {
  // Reading `json()` consumes the body; clone first so a plain-text proxy
  // error can still be forwarded. Test stubs often omit `clone`.
  const source = typeof response.clone === "function" ? response.clone() : response;

  try {
    const payload: unknown = await source.json();
    if (payload && typeof payload === "object") {
      const record = payload as Record<string, unknown>;
      for (const key of ["error", "message", "detail"] as const) {
        const value = record[key];
        if (typeof value === "string" && value.trim()) return value.trim();
      }
    }
  } catch {
    // Not JSON (or no `json` method on a test stub): try text below.
  }

  try {
    const text = await response.text();
    return text.trim() || undefined;
  } catch {
    return undefined;
  }
}

function isBodyInit(body: unknown): body is BodyInit {
  if (typeof body === "string") return true;
  if (typeof FormData !== "undefined" && body instanceof FormData) return true;
  if (typeof Blob !== "undefined" && body instanceof Blob) return true;
  if (typeof URLSearchParams !== "undefined" && body instanceof URLSearchParams) return true;
  if (typeof ArrayBuffer !== "undefined") {
    if (body instanceof ArrayBuffer || ArrayBuffer.isView(body)) return true;
  }
  if (typeof ReadableStream !== "undefined" && body instanceof ReadableStream) return true;
  return false;
}

/**
 * Send a proxy request and return the raw `Response`.
 *
 * Throws {@link ApiError} for every non-2xx answer, with the upstream message in
 * both `message` and `detail`.
 */
export async function browserFetchRaw(
  path: string,
  options: BrowserRequestOptions = {}
): Promise<Response> {
  const headers = new Headers(options.headers);
  let body: BodyInit | undefined;

  if (options.body !== undefined) {
    if (isBodyInit(options.body)) {
      body = options.body;
    } else {
      headers.set("Content-Type", "application/json");
      body = JSON.stringify(options.body);
    }
  }

  const response = await fetch(path, {
    method: options.method ?? (body === undefined ? "GET" : "POST"),
    headers,
    body,
    signal: options.signal,
    cache: options.cache,
  });

  if (response.ok === false || (typeof response.status === "number" && response.status >= 400)) {
    const detail = await errorDetail(response);
    throw new ApiError(
      response.status,
      detail ?? `Request failed (${response.status})`,
      detail
    );
  }

  return response;
}

/** Send a proxy request and parse the response as JSON (204/empty → undefined). */
export async function browserFetch<T>(
  path: string,
  options: BrowserRequestOptions = {}
): Promise<T> {
  const response = await browserFetchRaw(path, options);
  if (response.status === 204) return undefined as T;

  // A successful proxy may still have an empty body (e.g. a 200 with no
  // payload). Decode errors from an empty body become `undefined`; malformed
  // JSON is indistinguishable from an empty body with the web Response API.
  if (typeof response.json !== "function") return undefined as T;
  try {
    return (await response.json()) as T;
  } catch {
    return undefined as T;
  }
}

/** Send a proxy request and discard the response body. */
export async function browserFetchVoid(
  path: string,
  options: BrowserRequestOptions = {}
): Promise<void> {
  await browserFetchRaw(path, options);
}

/** Send a proxy request and read the response as text. */
export async function browserFetchText(
  path: string,
  options: BrowserRequestOptions = {}
): Promise<string> {
  const response = await browserFetchRaw(path, options);
  return response.text();
}

/**
 * Build `/api/path?key=value` with deterministic encoding.
 *
 * Nullish values are omitted; arrays repeat the key. Values are encoded with
 * `encodeURIComponent` (spaces become `%20`) so URLs stay stable for tests and
 * server logs.
 */
export function withQuery(
  path: string,
  params: Record<string, string | number | boolean | null | undefined | readonly (string | number)[]>
): string {
  const search: string[] = [];
  for (const [key, value] of Object.entries(params)) {
    if (value === null || value === undefined) continue;
    const values = Array.isArray(value) ? value : [value];
    for (const entry of values) {
      search.push(`${encodeURIComponent(key)}=${encodeURIComponent(String(entry))}`);
    }
  }
  if (search.length === 0) return path;
  return `${path}${path.includes("?") ? "&" : "?"}${search.join("&")}`;
}

/** Human-readable upstream message for a failed browser request. */
export function browserErrorMessage(error: unknown, fallback: string): string {
  if (error instanceof ApiError && error.detail) return error.detail;
  return fallback;
}
