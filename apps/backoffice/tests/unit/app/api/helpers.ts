import { NextRequest } from "next/server";
import { vi } from "vitest";

const BASE = "http://localhost:7082";

export function createApiMock() {
  return {
    apiFetch: vi.fn(),
    updateSetting: vi.fn(),
    config: vi.fn(() => ({ baseUrl: "http://api:7080", token: "test-token" })),
    listJobs: vi.fn(),
    listFolders: vi.fn(),
    fetchStats: vi.fn(),
    fetchSeriesMetadata: vi.fn(),
    updateSeries: vi.fn(),
    deleteSeries: vi.fn(),
    convertBook: vi.fn(),
    cancelJob: vi.fn(),
    updateBook: vi.fn(),
    updateReadingProgress: vi.fn(),
    updateLibraryMonitoring: vi.fn(),
    rebuildIndex: vi.fn(),
    rebuildThumbnails: vi.fn(),
    regenerateThumbnails: vi.fn(),
    startMetadataBatch: vi.fn(),
    startMetadataRefresh: vi.fn(),
    startMetadataRefreshAll: vi.fn(),
    startReadingStatusMatch: vi.fn(),
    startReadingStatusPush: vi.fn(),
    startReadingStatusPull: vi.fn(),
    startDownloadDetection: vi.fn(),
    startRssPoll: vi.fn(),
    markSeriesRead: vi.fn(),
    clearCache: vi.fn(),
    getCacheStats: vi.fn(),
    listKomgaReports: vi.fn(),
    getKomgaReport: vi.fn(),
  };
}

export function jsonRequest(
  path: string,
  options: {
    method?: string;
    body?: unknown;
    headers?: Record<string, string>;
    signal?: AbortSignal;
  } = {}
): NextRequest {
  const { method = "GET", body, headers, signal } = options;
  return new NextRequest(`${BASE}${path}`, {
    method,
    signal,
    body: body === undefined ? undefined : JSON.stringify(body),
    headers: { "content-type": "application/json", ...headers },
  });
}

export function routeCtx<T extends Record<string, unknown>>(params: T): { params: Promise<any> } {
  return { params: Promise.resolve(params) };
}

export async function bodyOf(res: Response): Promise<unknown> {
  return res.json();
}
