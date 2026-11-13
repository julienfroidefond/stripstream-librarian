// @vitest-environment node
import { describe, expect, it, vi } from "vitest";

import { jsonRequest, routeCtx } from "./helpers";

vi.mock("@/lib/api", async () => {
  const { createApiMock } = await import("./helpers");
  return createApiMock();
});

vi.mock("next/cache", () => ({ revalidatePath: vi.fn(), revalidateTag: vi.fn() }));

import { revalidatePath, revalidateTag } from "next/cache";
import { apiFetch } from "@/lib/api";
import { GET as listGenres } from "@/app/api/genres/route";
import { PATCH as patchGenre, DELETE as deleteGenre } from "@/app/api/genres/[name]/route";
import { POST as aiSuggest } from "@/app/api/genres/ai-suggest/route";
import { POST as assign } from "@/app/api/genres/assign/route";
import { GET as untagged } from "@/app/api/genres/untagged-series/route";
import { GET as browseGenres } from "@/app/api/genres/browse/route";
import { GET as readingOverview } from "@/app/api/reading-overview/route";
import { GET as readingOverviewSeriesBooks } from "@/app/api/reading-overview/[userId]/series-books/route";

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
    expect(vi.mocked(revalidateTag)).toHaveBeenCalledWith("series", { expire: 0 });
    expect(vi.mocked(revalidatePath)).toHaveBeenCalledWith("/genres");
  });

  it("DELETE encodes the genre name", async () => {
    mockApi.mockResolvedValue({ deleted: true });
    await deleteGenre(jsonRequest("/api/genres/Sci%20Fi", { method: "DELETE" }), routeCtx({ name: "Sci Fi" }));
    expect(mockApi).toHaveBeenCalledWith("/genres/Sci%20Fi", { method: "DELETE" });
    expect(vi.mocked(revalidateTag)).toHaveBeenCalledWith("series", { expire: 0 });
  });

  it("POST ai-suggest forwards body", async () => {
    mockApi.mockResolvedValue([]);
    expect((await aiSuggest(jsonRequest("/api/genres/ai-suggest", { method: "POST", body: { series: [] } }))).status).toBe(200);
  });

  it("POST assign forwards body", async () => {
    mockApi.mockResolvedValue({ ok: true });
    const body = { genre: "SF", series_ids: ["s1"] };
    expect((await assign(jsonRequest("/api/genres/assign", { method: "POST", body }))).status).toBe(200);
    expect(vi.mocked(revalidateTag)).toHaveBeenCalledWith("series:s1", { expire: 0 });
  });

  it("GET browse forwards the repeated-key query", async () => {
    mockApi.mockResolvedValue({ items: [], total: 0, page: 1, limit: 100, facets: { genres: [], untagged: 0, total: 0 } });
    await browseGenres(jsonRequest("/api/genres/browse?genres=A&genres=B&exclude=C&untagged=true"));
    expect(mockApi).toHaveBeenCalledWith("/genres/browse?genres=A&genres=B&exclude=C&untagged=true");
  });

  it("GET browse returns 500 on error", async () => {
    mockApi.mockRejectedValue(new Error("bad"));
    expect((await browseGenres(jsonRequest("/api/genres/browse"))).status).toBe(500);
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

  it("GET reading-overview series books forwards user and query", async () => {
    mockApi.mockResolvedValue([]);
    await readingOverviewSeriesBooks(
      jsonRequest("/api/reading-overview/u1/series-books?series_id=s1"),
      routeCtx({ userId: "u1" }),
    );
    expect(mockApi).toHaveBeenCalledWith("/admin/reading-overview/u1/series-books?series_id=s1");
  });
});
