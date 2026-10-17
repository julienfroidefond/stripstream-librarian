// @vitest-environment node
import { describe, expect, it, vi } from "vitest";

import { jsonRequest } from "./helpers";

vi.mock("@/lib/api", async () => {
  const { createApiMock } = await import("./helpers");
  return createApiMock();
});

import { fetchDiagnostics } from "@/lib/api";
import { GET as getDiagnostics } from "@/app/api/diagnostics/route";

describe("/api/diagnostics", () => {
  it("proxies the aggregated report", async () => {
    const report = {
      version: "1.2.3",
      uptime_seconds: 42,
      database: { reachable: true },
      cache: { hits: 1 },
      thumbnails: { file_count: 3 },
      jobs: { total: 1, recent_failures: [] },
      libraries: [],
    };
    vi.mocked(fetchDiagnostics).mockResolvedValue(report as never);

    const res = await getDiagnostics(jsonRequest("/api/diagnostics"));

    expect(fetchDiagnostics).toHaveBeenCalled();
    expect(res.status).toBe(200);
    expect(await res.json()).toEqual(report);
  });

  it("returns 500 when the API is unreachable", async () => {
    vi.mocked(fetchDiagnostics).mockRejectedValue(new Error("API /diagnostics failed (503)"));
    const res = await getDiagnostics(jsonRequest("/api/diagnostics"));
    expect(res.status).toBe(500);
  });
});
