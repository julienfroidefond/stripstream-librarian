import { afterEach, describe, expect, it, vi } from "vitest";

import { ApiError } from "@/lib/api/errors";
import {
  browserErrorMessage,
  browserFetch,
  browserFetchRaw,
  browserFetchVoid,
  browserFetchText,
  withQuery,
} from "@/lib/api/browser/transport";

afterEach(() => {
  vi.unstubAllGlobals();
});

function jsonResponse(payload: unknown, status = 200): Response {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

function stubFetch(response: Response | Promise<Response>) {
  const fetchMock = vi.fn(
    async (_input: RequestInfo | URL, _init?: RequestInit): Promise<Response> => response
  );
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

describe("browserFetch", () => {
  it("GETs and parses JSON by default", async () => {
    const fetchMock = stubFetch(jsonResponse({ id: "s1" }));

    await expect(browserFetch("/api/series/s1")).resolves.toEqual({ id: "s1" });
    expect(fetchMock).toHaveBeenCalledWith("/api/series/s1", {
      method: "GET",
      headers: expect.any(Headers),
      body: undefined,
      signal: undefined,
      cache: undefined,
    });
  });

  it("JSON-encodes a plain object body and infers POST", async () => {
    const fetchMock = stubFetch(jsonResponse({ ok: true }));

    await browserFetch("/api/series/s1", { body: { title: "Astérix" } });

    const init = fetchMock.mock.calls[0][1] as RequestInit;
    expect(init.method).toBe("POST");
    expect(init.body).toBe(JSON.stringify({ title: "Astérix" }));
    expect((init.headers as Headers).get("Content-Type")).toBe("application/json");
  });

  it("keeps explicit methods, headers and signals", async () => {
    const fetchMock = stubFetch(jsonResponse({}));
    const controller = new AbortController();

    await browserFetch("/api/series/s1", {
      method: "PUT",
      body: { rating: 4 },
      headers: { "X-Test": "1" },
      signal: controller.signal,
      cache: "no-store",
    });

    const init = fetchMock.mock.calls[0][1] as RequestInit;
    expect(init.method).toBe("PUT");
    expect(init.signal).toBe(controller.signal);
    expect(init.cache).toBe("no-store");
    expect((init.headers as Headers).get("X-Test")).toBe("1");
  });

  it("returns undefined for 204", async () => {
    stubFetch(new Response(null, { status: 204 }));
    await expect(browserFetch("/api/jobs/j1/cancel")).resolves.toBeUndefined();
  });

  it("throws ApiError with the proxy error envelope", async () => {
    stubFetch(jsonResponse({ error: "Series not found" }, 404));

    await expect(browserFetch("/api/series/missing")).rejects.toMatchObject({
      name: "ApiError",
      status: 404,
      message: "Series not found",
      detail: "Series not found",
    });
  });

  it("falls back to a plain-text error body", async () => {
    stubFetch(new Response("upstream unavailable", { status: 502 }));

    await expect(browserFetch("/api/series")).rejects.toThrow("upstream unavailable");
  });

  it("uses a generic message when the error body is empty", async () => {
    stubFetch(new Response(null, { status: 500 }));

    await expect(browserFetch("/api/series")).rejects.toMatchObject({
      status: 500,
      message: "Request failed (500)",
    });
  });

  it("works with minimal test stubs that only expose json()", async () => {
    const fetchMock = vi.fn(async () => ({ ok: true, status: 200, json: async () => [] }));
    vi.stubGlobal("fetch", fetchMock);

    await expect(browserFetch("/api/series")).resolves.toEqual([]);
  });

  it("returns undefined for a successful response with no body", async () => {
    stubFetch(new Response(null, { status: 200 }));

    await expect(browserFetch("/api/settings/anilist")).resolves.toBeUndefined();
  });
});

describe("browserFetchRaw", () => {
  it("returns the response for blob-style callers", async () => {
    const response = new Response("bytes", { status: 200 });
    stubFetch(response);

    await expect(browserFetchRaw("/api/books/b1/file")).resolves.toBe(response);
  });

  it("accepts a FormData body without forcing a JSON content type", async () => {
    const fetchMock = stubFetch(jsonResponse({ ok: true }));
    const body = new FormData();
    body.set("file", "value");

    await browserFetch("/api/upload", { method: "POST", body });

    const init = fetchMock.mock.calls[0][1] as RequestInit;
    expect(init.body).toBe(body);
    expect((init.headers as Headers).has("Content-Type")).toBe(false);
  });
});

describe("browserFetchVoid / browserFetchText", () => {
  it("discards the body", async () => {
    stubFetch(jsonResponse({ ignored: true }));
    await expect(browserFetchVoid("/api/delete", { method: "DELETE" })).resolves.toBeUndefined();
  });

  it("reads text", async () => {
    stubFetch(new Response("hello", { status: 200 }));
    await expect(browserFetchText("/api/text")).resolves.toBe("hello");
  });
});

describe("withQuery", () => {
  it("encodes values with %20 and omits nullish entries", () => {
    expect(
      withQuery("/api/series", { q: "Astérix le Gaulois", page: 2, library_id: null })
    ).toBe("/api/series?q=Ast%C3%A9rix%20le%20Gaulois&page=2");
  });

  it("repeats array keys and preserves an existing query string", () => {
    expect(withQuery("/api/series?limit=1", { genre: ["BD", "Manga"] })).toBe(
      "/api/series?limit=1&genre=BD&genre=Manga"
    );
  });

  it("returns the path unchanged when there is nothing to add", () => {
    expect(withQuery("/api/series", {})).toBe("/api/series");
  });
});

describe("browserErrorMessage", () => {
  it("prefers the upstream detail", () => {
    expect(browserErrorMessage(new ApiError(500, "boom", "upstream boom"), "fallback")).toBe(
      "upstream boom"
    );
  });

  it("falls back for network errors and unknown values", () => {
    expect(browserErrorMessage(new Error("network"), "fallback")).toBe("fallback");
    expect(browserErrorMessage("nope", "fallback")).toBe("fallback");
  });
});
