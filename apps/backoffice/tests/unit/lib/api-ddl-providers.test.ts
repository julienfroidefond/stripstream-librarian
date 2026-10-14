import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const cookieStore = vi.hoisted(() => ({ get: vi.fn() }));

vi.mock("next/headers", () => ({
  cookies: async () => cookieStore,
}));

import {
  dismissDdlProviderDownload,
  fetchDdlProviderDownloads,
  fetchDdlProviders,
  fetchDdlSearchSettings,
  hasEnabledDdlProvider,
  searchDdlProvider,
  startDdlProviderDownload,
  startDdlSearchAll,
  testDdlProvider,
  toggleDdlProvider,
  updateDdlSearchSettings,
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

type Call = {
  url: string;
  method: string;
  body: unknown;
  init: RequestInit & { next?: unknown };
};

function lastCall(): Call {
  const calls = fetchMock.mock.calls;
  const [url, init] = calls[calls.length - 1] as [string, RequestInit];
  return {
    url,
    method: init.method ?? "GET",
    body: init.body ? JSON.parse(init.body as string) : undefined,
    init,
  };
}

describe("download providers", () => {
  it("lists every provider", async () => {
    await fetchDdlProviders();
    expect(lastCall()).toMatchObject({
      url: "http://api:7080/ddl-providers",
      method: "GET",
    });
  });

  it("gates the toolbar action on an enabled provider", async () => {
    fetchMock.mockResolvedValueOnce(
      makeResponse([
        { id: "fourtoutici", enabled: false },
        { id: "annas", enabled: true },
      ])
    );
    await expect(hasEnabledDdlProvider()).resolves.toBe(true);
    expect(lastCall().init.next).toEqual({ revalidate: 60 });

    fetchMock.mockResolvedValueOnce(makeResponse([{ id: "fourtoutici", enabled: false }]));
    await expect(hasEnabledDdlProvider()).resolves.toBe(false);
  });

  it("treats a provider fetch failure as no provider", async () => {
    fetchMock.mockResolvedValueOnce(makeResponse("boom", 500));
    await expect(hasEnabledDdlProvider()).resolves.toBe(false);
  });

  it("toggles a provider and forwards the base URL", async () => {
    await toggleDdlProvider("fourtoutici", true, "https://mirror.example");
    expect(lastCall()).toMatchObject({
      url: "http://api:7080/ddl-providers/fourtoutici/toggle",
      method: "POST",
      body: { enabled: true, base_url: "https://mirror.example" },
    });
  });

  it("tests a provider with an optional query", async () => {
    await testDdlProvider("fourtoutici", "Astérix");
    expect(lastCall()).toMatchObject({
      url: "http://api:7080/ddl-providers/fourtoutici/test",
      method: "POST",
      body: { query: "Astérix" },
    });
  });

  it("searches a provider", async () => {
    await searchDdlProvider("fourtoutici", {
      query: "Astérix",
      series_name: "Astérix",
      missing_volumes: [41],
    });
    expect(lastCall()).toMatchObject({
      url: "http://api:7080/ddl-providers/fourtoutici/search",
      method: "POST",
      body: { query: "Astérix", series_name: "Astérix", missing_volumes: [41] },
    });
  });

  it("starts a download", async () => {
    await startDdlProviderDownload("fourtoutici", {
      external_id: "abc",
      filename: "Astérix T41.pdf",
      library_id: "lib-1",
      series_name: "Astérix",
      volume_number: 41,
    });
    expect(lastCall()).toMatchObject({
      url: "http://api:7080/ddl-providers/fourtoutici/downloads",
      method: "POST",
      body: {
        external_id: "abc",
        filename: "Astérix T41.pdf",
        library_id: "lib-1",
        series_name: "Astérix",
        volume_number: 41,
      },
    });
  });

  it("lists downloads across providers", async () => {
    await fetchDdlProviderDownloads();
    expect(lastCall()).toMatchObject({
      url: "http://api:7080/ddl-provider-downloads",
      method: "GET",
    });
  });

  it("reads the background-search cadence", async () => {
    await fetchDdlSearchSettings();
    expect(lastCall()).toMatchObject({
      url: "http://api:7080/ddl-providers/search-settings",
      method: "GET",
    });
    expect(lastCall().init.next).toEqual({ revalidate: 60 });
  });

  it("updates the background-search cadence", async () => {
    await updateDdlSearchSettings(60);
    expect(lastCall()).toMatchObject({
      url: "http://api:7080/ddl-providers/search-settings",
      method: "POST",
      body: { search_interval_minutes: 60 },
    });
  });

  it("starts a global background search", async () => {
    await startDdlSearchAll();
    expect(lastCall()).toMatchObject({
      url: "http://api:7080/ddl-providers/search-all",
      method: "POST",
    });
  });

  it("dismisses a download softly and hard-deletes on demand", async () => {
    await dismissDdlProviderDownload("dl-1");
    expect(lastCall()).toMatchObject({
      url: "http://api:7080/ddl-provider-downloads/dl-1",
      method: "DELETE",
    });

    await dismissDdlProviderDownload("dl-1", true);
    expect(lastCall()).toMatchObject({
      url: "http://api:7080/ddl-provider-downloads/dl-1?hard=true",
      method: "DELETE",
    });
  });
});
