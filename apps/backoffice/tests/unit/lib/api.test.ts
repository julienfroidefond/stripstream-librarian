import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const cookieStore = vi.hoisted(() => ({ get: vi.fn() }));

vi.mock("next/headers", () => ({
  cookies: async () => cookieStore,
}));

import { apiFetch, config, getBookCoverUrl } from "@/lib/api";

function makeResponse(body: unknown, status = 200): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    text: async () => (typeof body === "string" ? body : JSON.stringify(body)),
    json: async () => body,
  } as unknown as Response;
}

const env = {
  baseUrl: process.env.API_BASE_URL,
  token: process.env.API_BOOTSTRAP_TOKEN,
};

let fetchMock: ReturnType<typeof vi.fn>;

beforeEach(() => {
  process.env.API_BASE_URL = "http://api:7080/";
  process.env.API_BOOTSTRAP_TOKEN = "bootstrap-token";
  cookieStore.get.mockReset();
  cookieStore.get.mockReturnValue(undefined);
  fetchMock = vi.fn().mockResolvedValue(makeResponse({ ok: true }));
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
  if (env.baseUrl === undefined) delete process.env.API_BASE_URL;
  else process.env.API_BASE_URL = env.baseUrl;
  if (env.token === undefined) delete process.env.API_BOOTSTRAP_TOKEN;
  else process.env.API_BOOTSTRAP_TOKEN = env.token;
});

type FetchInit = RequestInit & { next?: unknown };

function lastFetchInit(): FetchInit {
  return fetchMock.mock.calls[0][1] as FetchInit;
}

describe("config", () => {
  it("strips the trailing slash from the base URL", () => {
    expect(config()).toEqual({ baseUrl: "http://api:7080", token: "bootstrap-token" });
  });

  it("defaults the base URL when unset", () => {
    delete process.env.API_BASE_URL;
    expect(config().baseUrl).toBe("http://api:7080");
  });

  it("throws when the bootstrap token is missing", () => {
    delete process.env.API_BOOTSTRAP_TOKEN;
    expect(() => config()).toThrow("API_BOOTSTRAP_TOKEN");
  });
});

describe("apiFetch", () => {
  it("calls the API with the path, auth header and no-store cache", async () => {
    await apiFetch("/libraries");
    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [url] = fetchMock.mock.calls[0];
    expect(url).toBe("http://api:7080/libraries");
    const init = lastFetchInit();
    expect((init.headers as Headers).get("Authorization")).toBe("Bearer bootstrap-token");
    expect(init.cache).toBe("no-store");
  });

  it("forwards the next options when provided", async () => {
    await apiFetch("/libraries", { next: { revalidate: 30 } });
    const init = lastFetchInit();
    expect(init.next).toEqual({ revalidate: 30 });
    expect(init.cache).toBeUndefined();
  });

  it("sets a JSON content type when a body is present", async () => {
    await apiFetch("/libraries", { method: "POST", body: JSON.stringify({ name: "x" }) });
    expect((lastFetchInit().headers as Headers).get("Content-Type")).toBe("application/json");
  });

  it("keeps an explicit content type", async () => {
    await apiFetch("/libraries", {
      method: "POST",
      body: "raw",
      headers: { "Content-Type": "text/plain" },
    });
    expect((lastFetchInit().headers as Headers).get("Content-Type")).toBe("text/plain");
  });

  it("injects X-As-User when the impersonation cookie is set", async () => {
    cookieStore.get.mockReturnValue({ value: "user-42" });
    await apiFetch("/libraries");
    expect((lastFetchInit().headers as Headers).get("X-As-User")).toBe("user-42");
    expect(cookieStore.get).toHaveBeenCalledWith("as_user_id");
  });

  it("does not inject X-As-User without the cookie", async () => {
    await apiFetch("/libraries");
    expect((lastFetchInit().headers as Headers).get("X-As-User")).toBeNull();
  });

  it("returns null for a 204 response", async () => {
    fetchMock.mockResolvedValue(makeResponse(null, 204));
    await expect(apiFetch("/tokens/1")).resolves.toBeNull();
  });

  it("parses the JSON body on success", async () => {
    fetchMock.mockResolvedValue(makeResponse({ id: "1", name: "lib" }));
    await expect(apiFetch("/libraries/1")).resolves.toEqual({ id: "1", name: "lib" });
  });

  it("throws with the path, status and body on error", async () => {
    fetchMock.mockResolvedValue(makeResponse("boom", 500));
    await expect(apiFetch("/libraries")).rejects.toThrow(
      "API /libraries failed (500): boom",
    );
  });
});

describe("getBookCoverUrl", () => {
  it("builds the thumbnail URL without a version", () => {
    expect(getBookCoverUrl("abc")).toBe("/api/books/abc/thumbnail");
  });

  it("appends an encoded version query when provided", () => {
    expect(getBookCoverUrl("abc", "2024-01-02 10:00")).toBe(
      "/api/books/abc/thumbnail?v=2024-01-02%2010%3A00",
    );
  });

  it("ignores a null or empty version", () => {
    expect(getBookCoverUrl("abc", null)).toBe("/api/books/abc/thumbnail");
    expect(getBookCoverUrl("abc", "")).toBe("/api/books/abc/thumbnail");
  });
});
