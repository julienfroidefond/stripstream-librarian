// @vitest-environment node
import { describe, expect, it, vi } from "vitest";

import { jsonRequest, routeCtx } from "./helpers";

vi.mock("@/lib/api", async () => {
  const { createApiMock } = await import("./helpers");
  return createApiMock();
});

import { apiFetch } from "@/lib/api";
import { GET as listGenres } from "@/app/api/genres/route";
import { PATCH as patchGenre, DELETE as deleteGenre } from "@/app/api/genres/[name]/route";
import { POST as aiSuggest } from "@/app/api/genres/ai-suggest/route";
import { POST as assign } from "@/app/api/genres/assign/route";
import { GET as untagged } from "@/app/api/genres/untagged-series/route";
import { GET as readingOverview } from "@/app/api/reading-overview/route";

const mockApi = vi.mocked(apiFetch);

describe("/api/genres", () => {
  it("GET without library filter", async () => {
    mockApi.mockResolvedValue([]);
    await listGenres(jsonRequest("/api/genres"));
    expect(mockApi).toHaveBeenCalledWith("/genres");
  });

  it("GET with library filter", async () => {
    mockApi.mockResolvedValue([]);
    await listGenres(jsonRequest("/api/genres?library_id=l1"));
    expect(mockApi).toHaveBeenCalledWith("/genres?library_id=l1");
  });

  it("GET returns 500 on error", async () => {
    mockApi.mockRejectedValue(new Error("bad"));
    expect((await listGenres(jsonRequest("/api/genres"))).status).toBe(500);
  });

  it("PATCH encodes the genre name", async () => {
    mockApi.mockResolvedValue({ ok: true });
    await patchGenre(jsonRequest("/api/genres/Sci%20Fi", { method: "PATCH", body: { name: "SF" } }), routeCtx({ name: "Sci Fi" }));
    expect(mockApi).toHaveBeenCalledWith("/genres/Sci%20Fi", expect.objectContaining({ method: "PATCH" }));
  });

  it("DELETE encodes the genre name", async () => {
    mockApi.mockResolvedValue({ deleted: true });
    await deleteGenre(jsonRequest("/api/genres/Sci%20Fi", { method: "DELETE" }), routeCtx({ name: "Sci Fi" }));
    expect(mockApi).toHaveBeenCalledWith("/genres/Sci%20Fi", { method: "DELETE" });
  });

  it("POST ai-suggest forwards body", async () => {
    mockApi.mockResolvedValue([]);
    expect((await aiSuggest(jsonRequest("/api/genres/ai-suggest", { method: "POST", body: { series: [] } }))).status).toBe(200);
  });

  it("POST assign forwards body", async () => {
    mockApi.mockResolvedValue({ ok: true });
    expect((await assign(jsonRequest("/api/genres/assign", { method: "POST", body: { genres: [] } }))).status).toBe(200);
  });

  it("GET untagged forwards query", async () => {
    mockApi.mockResolvedValue([]);
    await untagged(jsonRequest("/api/genres/untagged-series?limit=10"));
    expect(mockApi).toHaveBeenCalledWith("/genres/untagged-series?limit=10");
  });

  it("GET reading-overview proxies", async () => {
    mockApi.mockResolvedValue([]);
    await readingOverview();
    expect(mockApi).toHaveBeenCalledWith("/admin/reading-overview");
  });

  it("GET reading-overview returns 500 on error", async () => {
    mockApi.mockRejectedValue(new Error("bad"));
    expect((await readingOverview()).status).toBe(500);
  });
});
