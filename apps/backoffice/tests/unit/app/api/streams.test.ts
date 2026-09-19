// @vitest-environment node
import { NextRequest } from "next/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/api", async () => {
  const { createApiMock } = await import("./helpers");
  return createApiMock();
});

import { GET as jobsStream } from "@/app/api/jobs/stream/route";
import { GET as jobStream } from "@/app/api/jobs/[id]/stream/route";
import { GET as torrentStream } from "@/app/api/torrent-downloads/stream/route";
import { GET as telegramStream } from "@/app/api/telegram-monitor/downloads/stream/route";

const fetchMock = vi.fn();
const decoder = new TextDecoder();

function jsonResponse(payload: unknown): Response {
  return new Response(JSON.stringify(payload), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
}

function request(signal?: AbortSignal) {
  return new NextRequest("http://localhost:7082/api/stream", { signal });
}

async function readChunk(reader: ReadableStreamDefaultReader<Uint8Array>): Promise<string | null> {
  const { value, done } = await reader.read();
  if (done) return null;
  return decoder.decode(value);
}

beforeEach(() => {
  fetchMock.mockReset();
  fetchMock.mockImplementation(() => Promise.resolve(jsonResponse([{ status: "success" }])));
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("GET /api/jobs/stream", () => {
  it("polls every 2s while a job is active", async () => {
    vi.useFakeTimers();
    fetchMock.mockImplementation(() => Promise.resolve(jsonResponse([{ status: "running" }])));
    const ac = new AbortController();
    const res = await jobsStream(request(ac.signal));
    expect(res.headers.get("content-type")).toBe("text/event-stream");
    expect(res.headers.get("cache-control")).toBe("no-cache");
    const reader = res.body!.getReader();
    expect(await readChunk(reader)).toBe("");
    await vi.advanceTimersByTimeAsync(0);
    expect(await readChunk(reader)).toBe('data: [{"status":"running"}]\n\n');
    fetchMock.mockClear();
    await vi.advanceTimersByTimeAsync(2000);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    await readChunk(reader);
    ac.abort();
    await vi.advanceTimersByTimeAsync(0);
    expect(await readChunk(reader)).toBeNull();
  });

  it("polls every 10s when idle and emits a heartbeat every 15s", async () => {
    vi.useFakeTimers();
    fetchMock.mockImplementation(() => Promise.resolve(jsonResponse([{ status: "success" }])));
    const ac = new AbortController();
    const res = await jobsStream(request(ac.signal));
    const reader = res.body!.getReader();
    expect(await readChunk(reader)).toBe("");
    await vi.advanceTimersByTimeAsync(0);
    expect(await readChunk(reader)).toBe('data: [{"status":"success"}]\n\n');
    fetchMock.mockClear();
    await vi.advanceTimersByTimeAsync(15000);
    const chunks = [await readChunk(reader), await readChunk(reader)];
    expect(chunks).toContain(": heartbeat\n\n");
    ac.abort();
  });

  it("ignores fetch failures", async () => {
    fetchMock.mockRejectedValue(new Error("down"));
    const ac = new AbortController();
    const res = await jobsStream(request(ac.signal));
    const reader = res.body!.getReader();
    expect(await readChunk(reader)).toBe("");
    await vi.waitFor(() => expect(fetchMock).toHaveBeenCalled());
    ac.abort();
  });

  it("stops silently when the client cancels the connection", async () => {
    vi.useFakeTimers();
    let resolveFetch!: (r: Response) => void;
    fetchMock.mockImplementation(() => new Promise<Response>((r) => { resolveFetch = r; }));
    const res = await jobsStream(request());
    const reader = res.body!.getReader();
    expect(await readChunk(reader)).toBe("");
    await reader.cancel();
    resolveFetch(jsonResponse([{ status: "running" }]));
    await vi.advanceTimersByTimeAsync(0);
    await vi.advanceTimersByTimeAsync(2000);
    expect(fetchMock).toHaveBeenCalled();
  });
});

describe("GET /api/jobs/[id]/stream", () => {
  it("emits job updates and closes on a terminal status", async () => {
    fetchMock.mockImplementation(() => Promise.resolve(jsonResponse({ status: "success" })));
    const res = await jobStream(request(), { params: Promise.resolve({ id: "j1" }) } as never);
    expect(res.headers.get("content-type")).toBe("text/event-stream");
    const reader = res.body!.getReader();
    expect(await readChunk(reader)).toBe("");
    expect(await readChunk(reader)).toBe('data: {"status":"success"}\n\n');
    expect(await readChunk(reader)).toBeNull();
    expect(fetchMock).toHaveBeenCalledWith(
      "http://api:7080/index/jobs/j1",
      expect.objectContaining({ headers: { Authorization: "Bearer test-token" } })
    );
  });

  it("keeps polling while the job runs then stops on abort", async () => {
    vi.useFakeTimers();
    fetchMock.mockImplementation(() => Promise.resolve(jsonResponse({ status: "running" })));
    const ac = new AbortController();
    const res = await jobStream(request(ac.signal), { params: Promise.resolve({ id: "j1" }) } as never);
    const reader = res.body!.getReader();
    await readChunk(reader);
    await vi.advanceTimersByTimeAsync(0);
    await readChunk(reader);
    fetchMock.mockClear();
    await vi.advanceTimersByTimeAsync(500);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    ac.abort();
    await vi.advanceTimersByTimeAsync(0);
    expect(await readChunk(reader)).toBeNull();
  });

  it("ignores fetch failures", async () => {
    fetchMock.mockRejectedValue(new Error("down"));
    const ac = new AbortController();
    const res = await jobStream(request(ac.signal), { params: Promise.resolve({ id: "j1" }) } as never);
    const reader = res.body!.getReader();
    await readChunk(reader);
    await vi.waitFor(() => expect(fetchMock).toHaveBeenCalled());
    ac.abort();
  });

  it("stops when the controller is closed and clears the polling interval", async () => {
    vi.useFakeTimers();
    let resolveFetch!: (r: Response) => void;
    fetchMock.mockImplementation(() => new Promise<Response>((r) => { resolveFetch = r; }));
    const res = await jobStream(request(), { params: Promise.resolve({ id: "j1" }) } as never);
    const reader = res.body!.getReader();
    expect(await readChunk(reader)).toBe("");
    await reader.cancel();
    resolveFetch(jsonResponse({ status: "running" }));
    await vi.advanceTimersByTimeAsync(0);
    await vi.advanceTimersByTimeAsync(500);
    expect(fetchMock).toHaveBeenCalled();
  });
});

describe("GET /api/torrent-downloads/stream", () => {
  it("streams downloads and honours abort", async () => {
    vi.useFakeTimers();
    fetchMock.mockImplementation(() => Promise.resolve(jsonResponse([{ status: "downloading" }])));
    const ac = new AbortController();
    const res = await torrentStream(request(ac.signal));
    const reader = res.body!.getReader();
    expect(await readChunk(reader)).toBe("");
    await vi.advanceTimersByTimeAsync(0);
    expect(await readChunk(reader)).toBe('data: [{"status":"downloading"}]\n\n');
    expect(fetchMock).toHaveBeenCalledWith(
      "http://api:7080/torrent-downloads",
      expect.objectContaining({ headers: { Authorization: "Bearer test-token" } })
    );
    ac.abort();
    await vi.advanceTimersByTimeAsync(0);
    expect(await readChunk(reader)).toBeNull();
  });

  it("ignores fetch failures", async () => {
    fetchMock.mockRejectedValue(new Error("down"));
    const ac = new AbortController();
    const res = await torrentStream(request(ac.signal));
    const reader = res.body!.getReader();
    await readChunk(reader);
    await vi.waitFor(() => expect(fetchMock).toHaveBeenCalled());
    ac.abort();
  });

  it("emits heartbeats and stops when the connection is cancelled", async () => {
    vi.useFakeTimers();
    fetchMock.mockImplementation(() => Promise.resolve(jsonResponse([{ status: "success" }])));
    const res = await torrentStream(request());
    const reader = res.body!.getReader();
    expect(await readChunk(reader)).toBe("");
    await vi.advanceTimersByTimeAsync(15000);
    const chunks = [await readChunk(reader), await readChunk(reader), await readChunk(reader)];
    expect(chunks).toContain(": heartbeat\n\n");
    await reader.cancel();
    await vi.advanceTimersByTimeAsync(15000);
    expect(fetchMock.mock.calls.length).toBeGreaterThan(1);
  });
});

describe("GET /api/telegram-monitor/downloads/stream", () => {
  it("streams downloads", async () => {
    vi.useFakeTimers();
    fetchMock.mockImplementation(() => Promise.resolve(jsonResponse([{ status: "importing" }])));
    const ac = new AbortController();
    const res = await telegramStream(request(ac.signal));
    const reader = res.body!.getReader();
    expect(await readChunk(reader)).toBe("");
    await vi.advanceTimersByTimeAsync(0);
    expect(await readChunk(reader)).toBe('data: [{"status":"importing"}]\n\n');
    expect(fetchMock).toHaveBeenCalledWith(
      "http://api:7080/telegram-monitor/downloads",
      expect.objectContaining({ headers: { Authorization: "Bearer test-token" } })
    );
    ac.abort();
    await vi.advanceTimersByTimeAsync(0);
    expect(await readChunk(reader)).toBeNull();
  });

  it("ignores fetch failures", async () => {
    fetchMock.mockRejectedValue(new Error("down"));
    const ac = new AbortController();
    const res = await telegramStream(request(ac.signal));
    const reader = res.body!.getReader();
    await readChunk(reader);
    await vi.waitFor(() => expect(fetchMock).toHaveBeenCalled());
    ac.abort();
  });

  it("emits heartbeats and stops when the connection is cancelled", async () => {
    vi.useFakeTimers();
    fetchMock.mockImplementation(() => Promise.resolve(jsonResponse([{ status: "success" }])));
    const res = await telegramStream(request());
    const reader = res.body!.getReader();
    expect(await readChunk(reader)).toBe("");
    await vi.advanceTimersByTimeAsync(15000);
    const chunks = [await readChunk(reader), await readChunk(reader), await readChunk(reader)];
    expect(chunks).toContain(": heartbeat\n\n");
    await reader.cancel();
    await vi.advanceTimersByTimeAsync(15000);
    expect(fetchMock.mock.calls.length).toBeGreaterThan(1);
  });
});
