// @vitest-environment node

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("next/headers", () => ({
  cookies: async () => ({ get: () => undefined }),
}));

import { fetchApiVersion, fetchIndexerVersion, indexerBaseUrl } from "@/lib/api/versions";

const savedIndexerUrl = process.env.INDEXER_BASE_URL;

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), { status });
}

let fetchMock: ReturnType<typeof vi.fn>;

beforeEach(() => {
  process.env.API_BASE_URL = "http://api:7080";
  process.env.API_BOOTSTRAP_TOKEN = "bootstrap-token";
  delete process.env.INDEXER_BASE_URL;
  fetchMock = vi.fn();
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
  if (savedIndexerUrl === undefined) delete process.env.INDEXER_BASE_URL;
  else process.env.INDEXER_BASE_URL = savedIndexerUrl;
});

describe("indexerBaseUrl", () => {
  it("defaults to the Docker service and honours INDEXER_BASE_URL", () => {
    expect(indexerBaseUrl()).toBe("http://indexer:7081");
    process.env.INDEXER_BASE_URL = "http://localhost:7081/";
    expect(indexerBaseUrl()).toBe("http://localhost:7081");
  });
});

describe("fetchIndexerVersion", () => {
  it("reads one configured host, without trying a list of guesses", async () => {
    process.env.INDEXER_BASE_URL = "http://idx:9000";
    fetchMock.mockResolvedValue(json({ indexer: "3.31.0" }));

    await expect(fetchIndexerVersion()).resolves.toBe("3.31.0");
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock.mock.calls[0][0]).toBe("http://idx:9000/version");
    expect(fetchMock.mock.calls[0][1]).toMatchObject({ cache: "no-store" });
  });

  it("rejects on an error status, a network failure or a missing field", async () => {
    fetchMock.mockResolvedValueOnce(json({}, 503));
    await expect(fetchIndexerVersion()).rejects.toThrow("503");

    fetchMock.mockRejectedValueOnce(new Error("ECONNREFUSED"));
    await expect(fetchIndexerVersion()).rejects.toThrow("ECONNREFUSED");

    fetchMock.mockResolvedValueOnce(json({ version: "x" }));
    await expect(fetchIndexerVersion()).rejects.toThrow("indexer");
  });
});

describe("fetchApiVersion", () => {
  it("returns the api field of GET /version", async () => {
    fetchMock.mockResolvedValue(json({ api: "3.32.0" }));

    await expect(fetchApiVersion()).resolves.toBe("3.32.0");
    expect(fetchMock.mock.calls[0][0]).toBe("http://api:7080/version");
  });

  it("rejects instead of inventing a version", async () => {
    fetchMock.mockResolvedValue(json({}));
    await expect(fetchApiVersion()).rejects.toThrow("api");
  });
});
