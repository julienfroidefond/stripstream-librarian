// @vitest-environment node
import { describe, expect, it, vi } from "vitest";

import { jsonRequest, routeCtx } from "./helpers";

vi.mock("@/lib/api", async () => {
  const { createApiMock } = await import("./helpers");
  return createApiMock();
});

import { apiFetch, clearCache, getCacheStats, updateSetting } from "@/lib/api";
import { GET as getSetting, POST as postSetting } from "@/app/api/settings/[key]/route";
import { POST as clear } from "@/app/api/settings/cache/clear/route";
import { GET as stats } from "@/app/api/settings/cache/stats/route";
import { POST as testAiTagging } from "@/app/api/settings/ai_tagging/test/route";
import { GET as listMappings, POST as createMapping } from "@/app/api/settings/status-mappings/route";
import { DELETE as deleteMapping } from "@/app/api/settings/status-mappings/[id]/route";

const mockApi = vi.mocked(apiFetch);

describe("/api/settings/[key]", () => {
  it("GET proxies the setting", async () => {
    mockApi.mockResolvedValue({ value: 42 });
    const res = await getSetting(jsonRequest("/api/settings/foo"), routeCtx({ key: "foo" }));
    expect(mockApi).toHaveBeenCalledWith("/settings/foo");
    expect(await res.json()).toEqual({ value: 42 });
  });

  it("GET returns 500 on error", async () => {
    mockApi.mockRejectedValue(new Error("bad"));
    expect((await getSetting(jsonRequest("/api/settings/foo"), routeCtx({ key: "foo" }))).status).toBe(500);
  });

  it("POST delegates to updateSetting", async () => {
    vi.mocked(updateSetting).mockResolvedValue({ ok: true });
    const res = await postSetting(
      jsonRequest("/api/settings/foo", { method: "POST", body: { value: "bar" } }),
      routeCtx({ key: "foo" })
    );
    expect(updateSetting).toHaveBeenCalledWith("foo", "bar");
    expect(res.status).toBe(200);
  });

  it("POST returns 500 on error", async () => {
    vi.mocked(updateSetting).mockRejectedValue(new Error("bad"));
    expect(
      (await postSetting(jsonRequest("/api/settings/foo", { method: "POST", body: { value: 1 } }), routeCtx({ key: "foo" }))).status
    ).toBe(500);
  });
});

describe("/api/settings/cache", () => {
  it("POST clear delegates to clearCache", async () => {
    vi.mocked(clearCache).mockResolvedValue({ cleared: true } as never);
    const res = await clear();
    expect(clearCache).toHaveBeenCalled();
    expect(res.status).toBe(200);
  });

  it("POST clear returns 500 on error", async () => {
    vi.mocked(clearCache).mockRejectedValue(new Error("bad"));
    expect((await clear()).status).toBe(500);
  });

  it("GET stats delegates to getCacheStats", async () => {
    vi.mocked(getCacheStats).mockResolvedValue({ hits: 1 } as never);
    expect(await (await stats()).json()).toEqual({ hits: 1 });
  });

  it("GET stats returns 500 on error", async () => {
    vi.mocked(getCacheStats).mockRejectedValue(new Error("bad"));
    expect((await stats()).status).toBe(500);
  });
});

describe("/api/settings/ai_tagging/test", () => {
  it("POST forwards the body", async () => {
    mockApi.mockResolvedValue({ ok: true });
    const res = await testAiTagging(jsonRequest("/api/settings/ai_tagging/test", { method: "POST", body: { provider: "openai" } }));
    expect(mockApi).toHaveBeenCalledWith("/settings/ai_tagging/test", expect.objectContaining({ method: "POST" }));
    expect(res.status).toBe(200);
  });
});

describe("/api/settings/status-mappings", () => {
  it("GET lists mappings", async () => {
    mockApi.mockResolvedValue([{ id: "1" }]);
    expect(await (await listMappings()).json()).toEqual([{ id: "1" }]);
  });

  it("POST creates a mapping", async () => {
    mockApi.mockResolvedValue({ id: "2" });
    const res = await createMapping(jsonRequest("/api/settings/status-mappings", { method: "POST", body: { provider: "x" } }));
    expect(mockApi).toHaveBeenCalledWith("/settings/status-mappings", expect.objectContaining({ method: "POST" }));
    expect(res.status).toBe(200);
  });

  it("DELETE removes a mapping", async () => {
    mockApi.mockResolvedValue({ deleted: true });
    const res = await deleteMapping(jsonRequest("/api/settings/status-mappings/1", { method: "DELETE" }), routeCtx({ id: "1" }));
    expect(mockApi).toHaveBeenCalledWith("/settings/status-mappings/1", { method: "DELETE" });
    expect(res.status).toBe(200);
  });
});
