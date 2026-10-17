// @vitest-environment node
import { describe, expect, it, vi } from "vitest";

import { jsonRequest } from "./helpers";

vi.mock("@/lib/api", async () => {
  const { createApiMock } = await import("./helpers");
  return createApiMock();
});

import { fetchDiagnosticsProbe } from "@/lib/api";
import { GET as runProbe } from "@/app/api/diagnostics/probe/route";

describe("/api/diagnostics/probe", () => {
  it("proxies the on-demand probe", async () => {
    const probe = {
      sampled_at: "2026-01-01T00:00:00Z",
      integrations: [{ id: "prowlarr", configured: true, reachable: true, latency_ms: 12, detail: null }],
      missing_files: { checked: 42, missing: 0, samples: [] },
    };
    vi.mocked(fetchDiagnosticsProbe).mockResolvedValue(probe as never);

    const res = await runProbe(jsonRequest("/api/diagnostics/probe"));

    expect(fetchDiagnosticsProbe).toHaveBeenCalled();
    expect(res.status).toBe(200);
    expect(await res.json()).toEqual(probe);
  });

  it("returns 500 when the probe fails", async () => {
    vi.mocked(fetchDiagnosticsProbe).mockRejectedValue(new Error("API /diagnostics/probe failed (503)"));
    const res = await runProbe(jsonRequest("/api/diagnostics/probe"));
    expect(res.status).toBe(500);
  });
});
