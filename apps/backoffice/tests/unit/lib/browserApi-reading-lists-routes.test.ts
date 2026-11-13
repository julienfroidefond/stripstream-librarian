// @vitest-environment node
import { afterEach, describe, expect, it, vi } from "vitest";

import { readingListsApi } from "@/lib/api/browser/reading-lists";

import { exportedMethods, resolveProxyRoute } from "../helpers/proxyRoutes";

/**
 * Every call `readingListsApi` makes must land on a Next route handler that
 * exports the method used. Client tests mock `fetch` and route tests call the
 * handler directly, so neither half alone catches a URL no route serves (the
 * `/move` regression).
 */

type Call = { url: string; method: string };

function recordCalls() {
  const calls: Call[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      calls.push({ url: String(input), method: init?.method ?? "GET" });
      return new Response(JSON.stringify({}), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    })
  );
  return calls;
}

afterEach(() => {
  vi.unstubAllGlobals();
});

const invocations: Array<[string, () => Promise<unknown>]> = [
  ["list", () => readingListsApi.list("s1")],
  ["detail", () => readingListsApi.detail("l1")],
  ["create", () => readingListsApi.create("name")],
  ["update", () => readingListsApi.update("l1", { name: "n" })],
  ["remove", () => readingListsApi.remove("l1")],
  ["addSeries", () => readingListsApi.addSeries("l1", "s1")],
  ["removeSeries", () => readingListsApi.removeSeries("l1", "s1")],
  ["moveSeries", () => readingListsApi.moveSeries("l1", "s1", "l2")],
  ["reorder", () => readingListsApi.reorder("l1", ["s1"])],
  ["aiSuggest", () => readingListsApi.aiSuggest(3)],
];

describe("readingListsApi ↔ proxy routes", () => {
  it("covers every function of the client", () => {
    expect(invocations.map(([name]) => name).sort()).toEqual(
      Object.keys(readingListsApi).sort()
    );
  });

  it.each(invocations)("%s targets an existing route exporting its method", async (_name, call) => {
    const calls = recordCalls();
    await call();

    expect(calls).toHaveLength(1);
    const [{ url, method }] = calls;
    const file = resolveProxyRoute(url);
    expect(file, `no route handles ${method} ${url}`).not.toBeNull();
    expect(exportedMethods(file!)).toContain(method);
  });

  it("routes a move to the dedicated move handler", async () => {
    const calls = recordCalls();
    await readingListsApi.moveSeries("l1", "s1", "l2");
    expect(resolveProxyRoute(calls[0].url)).toMatch(
      /reading-lists\/\[id\]\/series\/\[seriesId\]\/move\/route\.ts$/
    );
  });
});

describe("proxy route resolver", () => {
  it("returns null for a path no route serves", () => {
    expect(resolveProxyRoute("/api/reading-lists/l1/series/s1/nope")).toBeNull();
  });

  it("prefers a static segment over a dynamic one", () => {
    expect(resolveProxyRoute("/api/reading-lists/l1/series/reorder")).toMatch(
      /series\/reorder\/route\.ts$/
    );
  });
});
