import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const cookieStore = vi.hoisted(() => ({ get: vi.fn() }));

vi.mock("next/headers", () => ({
  cookies: async () => cookieStore,
}));

import {
  cancelJob,
  clearCache,
  convertBook,
  createUser,
  deleteStatusMapping,
  deleteToken,
  deleteUser,
  fetchSeriesRecommendations,
  fetchSeriesStatuses,
  fetchStatsBreakdown,
  fetchStatsOverview,
  fetchReadingProgress,
  fetchStatusMappings,
  fetchUserGenreRestrictions,
  fetchUsers,
  getCacheStats,
  getSettings,
  getThumbnailStats,
  listJobs,
  listTokens,
  rebuildThumbnails,
  regenerateThumbnails,
  revokeToken,
  setUserGenreRestrictions,
  updateUser,
  upsertStatusMapping,
} from "@/lib/api";

function makeResponse(body: unknown, status = 200): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    text: async () => (typeof body === "string" ? body : JSON.stringify(body)),
    json: async () => body,
  } as unknown as Response;
}

let fetchMock: ReturnType<typeof vi.fn>;

beforeEach(() => {
  process.env.API_BASE_URL = "http://api:7080";
  process.env.API_BOOTSTRAP_TOKEN = "bootstrap-token";
  cookieStore.get.mockReset();
  cookieStore.get.mockReturnValue(undefined);
  fetchMock = vi.fn().mockResolvedValue(makeResponse({ ok: true }));
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

type Call = { url: string; method: string; body: unknown; init: RequestInit };

function lastCall(): Call {
  const calls = fetchMock.mock.calls;
  const [url, init] = calls[calls.length - 1] as [string, RequestInit];
  return {
    url: String(url),
    method: init.method ?? "GET",
    body: init.body ? JSON.parse(init.body as string) : undefined,
    init,
  };
}

describe("index jobs admin", () => {
  it("lists jobs", async () => {
    await listJobs();
    expect(lastCall()).toMatchObject({ url: "http://api:7080/index/status", method: "GET" });
  });

  it("rebuilds thumbnails with and without a library", async () => {
    await rebuildThumbnails();
    expect(lastCall()).toMatchObject({ url: "http://api:7080/index/thumbnails/rebuild", method: "POST", body: {} });
    await rebuildThumbnails("l1");
    expect(lastCall().body).toEqual({ library_id: "l1" });
  });

  it("regenerates thumbnails with and without a library", async () => {
    await regenerateThumbnails();
    expect(lastCall()).toMatchObject({ url: "http://api:7080/index/thumbnails/regenerate", body: {} });
    await regenerateThumbnails("l1");
    expect(lastCall().body).toEqual({ library_id: "l1" });
  });

  it("cancels a job", async () => {
    await cancelJob("j1");
    expect(lastCall()).toMatchObject({ url: "http://api:7080/index/cancel/j1", method: "POST" });
  });
});

describe("admin users and tokens", () => {
  it("lists tokens", async () => {
    await listTokens();
    expect(lastCall().url).toBe("http://api:7080/admin/tokens");
  });

  it("lists users with revalidation", async () => {
    await fetchUsers();
    expect(lastCall().init.next).toMatchObject({ revalidate: 60 });
  });

  it("creates, renames and deletes users", async () => {
    await createUser("bob");
    expect(lastCall()).toMatchObject({ url: "http://api:7080/admin/users", method: "POST", body: { username: "bob" } });

    await updateUser("u1", "bob2");
    expect(lastCall()).toMatchObject({ url: "http://api:7080/admin/users/u1", method: "PATCH", body: { username: "bob2" } });

    await deleteUser("u1");
    expect(lastCall()).toMatchObject({ url: "http://api:7080/admin/users/u1", method: "DELETE" });
  });

  it("reads and writes genre restrictions", async () => {
    await fetchUserGenreRestrictions("u1");
    expect(lastCall().url).toBe("http://api:7080/admin/users/u1/genre-restrictions");

    await setUserGenreRestrictions("u1", ["Action"]);
    expect(lastCall()).toMatchObject({
      url: "http://api:7080/admin/users/u1/genre-restrictions",
      method: "PUT",
      body: { blocked_genres: ["Action"] },
    });
  });

  it("revokes and deletes tokens", async () => {
    await revokeToken("t1");
    expect(lastCall()).toMatchObject({ url: "http://api:7080/admin/tokens/t1", method: "DELETE" });

    await deleteToken("t1");
    expect(lastCall()).toMatchObject({ url: "http://api:7080/admin/tokens/t1/delete", method: "POST" });
  });
});

describe("series recommendations and statuses", () => {
  it("fetches recommendations with pagination params", async () => {
    await fetchSeriesRecommendations(5, 2);
    expect(lastCall().url).toBe("http://api:7080/series/recommendations?limit=5&sources=2");
  });

  it("fetches statuses", async () => {
    await fetchSeriesStatuses();
    expect(lastCall()).toMatchObject({ url: "http://api:7080/series/statuses" });
  });
});

describe("settings and cache", () => {
  it("gets settings", async () => {
    await getSettings();
    expect(lastCall().init.cache).toBe("no-store");
  });

  it("gets cache and thumbnail stats", async () => {
    await getCacheStats();
    expect(lastCall().url).toBe("http://api:7080/settings/cache/stats");
    await getThumbnailStats();
    expect(lastCall().url).toBe("http://api:7080/settings/thumbnail/stats");
  });

  it("clears the cache", async () => {
    await clearCache();
    expect(lastCall()).toMatchObject({ url: "http://api:7080/settings/cache/clear", method: "POST" });
  });
});

describe("status mappings", () => {
  it("lists mappings", async () => {
    await fetchStatusMappings();
    expect(lastCall().url).toBe("http://api:7080/settings/status-mappings");
  });

  it("upserts a mapping", async () => {
    await upsertStatusMapping("ongoing", "reading");
    expect(lastCall()).toMatchObject({
      method: "POST",
      body: { provider_status: "ongoing", mapped_status: "reading" },
    });
  });

  it("deletes a mapping", async () => {
    await deleteStatusMapping("m1");
    expect(lastCall()).toMatchObject({ url: "http://api:7080/settings/status-mappings/m1", method: "DELETE" });
  });
});

describe("books and stats", () => {
  it("converts a book", async () => {
    await convertBook("b1");
    expect(lastCall()).toMatchObject({ url: "http://api:7080/books/b1/convert", method: "POST" });
  });

  it("fetches reading progress", async () => {
    await fetchReadingProgress("b1");
    expect(lastCall().url).toBe("http://api:7080/books/b1/progress");
  });

  it("fetches stats overview and breakdown", async () => {
    await fetchStatsOverview();
    expect(lastCall()).toMatchObject({ url: "http://api:7080/stats/overview" });
    await fetchStatsBreakdown();
    expect(lastCall().url).toBe("http://api:7080/stats/breakdown");
  });
});
