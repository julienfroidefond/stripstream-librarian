// @vitest-environment node
import { describe, expect, it, vi } from "vitest";

import { jsonRequest } from "./helpers";

vi.mock("@/lib/api", async () => {
  const { createApiMock } = await import("./helpers");
  return createApiMock();
});
vi.mock("next/cache", () => ({ revalidatePath: vi.fn(), revalidateTag: vi.fn() }));

import { apiFetch } from "@/lib/api";
import { POST as addToLibrary } from "@/app/api/discovery/add-to-library/route";
import { GET as hidden } from "@/app/api/discovery/hidden/route";
import { POST as hide } from "@/app/api/discovery/hide/route";
import { GET as prowlarr } from "@/app/api/discovery/prowlarr/route";
import { GET as trending } from "@/app/api/discovery/trending/route";
import { POST as unhide } from "@/app/api/discovery/unhide/route";
import { GET as latestFound } from "@/app/api/download-detection/latest-found/route";

const mockApi = vi.mocked(apiFetch);

describe("/api/discovery", () => {
  it("POST add-to-library forwards body", async () => {
    mockApi.mockResolvedValue({ ok: true });
    const res = await addToLibrary(jsonRequest("/api/discovery/add-to-library", { method: "POST", body: { id: 1 } }));
    expect(mockApi).toHaveBeenCalledWith("/discovery/add-to-library", expect.objectContaining({ method: "POST" }));
    expect(res.status).toBe(200);
  });

  it("POST add-to-library returns 500 on error", async () => {
    mockApi.mockRejectedValue(new Error("bad"));
    expect((await addToLibrary(jsonRequest("/api/discovery/add-to-library", { method: "POST", body: {} }))).status).toBe(500);
  });

  it("GET hidden proxies", async () => {
    mockApi.mockResolvedValue([]);
    expect((await hidden()).status).toBe(200);
  });

  it("POST hide forwards body", async () => {
    mockApi.mockResolvedValue({ ok: true });
    expect((await hide(jsonRequest("/api/discovery/hide", { method: "POST", body: { id: 1 } }))).status).toBe(200);
  });

  it("POST unhide forwards body", async () => {
    mockApi.mockResolvedValue({ ok: true });
    expect((await unhide(jsonRequest("/api/discovery/unhide", { method: "POST", body: { id: 1 } }))).status).toBe(200);
  });

  it("GET prowlarr forwards supported filters", async () => {
    mockApi.mockResolvedValue([]);
    await prowlarr(jsonRequest("/api/discovery/prowlarr?nocache=1&sort=seeders&indexer=x&category=y"));
    expect(mockApi).toHaveBeenCalledWith("/discovery/prowlarr?nocache=true&sort=seeders&indexer=x&category=y");
  });

  it("GET prowlarr omits empty filters", async () => {
    mockApi.mockResolvedValue([]);
    await prowlarr(jsonRequest("/api/discovery/prowlarr"));
    expect(mockApi).toHaveBeenCalledWith("/discovery/prowlarr?");
  });

  it("GET prowlarr returns 500 on error", async () => {
    mockApi.mockRejectedValue(new Error("bad"));
    expect((await prowlarr(jsonRequest("/api/discovery/prowlarr"))).status).toBe(500);
  });

  it("GET trending uses defaults", async () => {
    mockApi.mockResolvedValue([]);
    await trending(jsonRequest("/api/discovery/trending"));
    expect(mockApi).toHaveBeenCalledWith("/discovery/trending?provider=anilist&limit=24&offset=0");
  });

  it("GET trending appends optional period and nocache", async () => {
    mockApi.mockResolvedValue([]);
    await trending(jsonRequest("/api/discovery/trending?provider=mal&limit=5&offset=10&period=week&nocache=1"));
    expect(mockApi).toHaveBeenCalledWith("/discovery/trending?provider=mal&limit=5&offset=10&period=week&nocache=true");
  });

  it("GET latest-found proxies", async () => {
    mockApi.mockResolvedValue({ id: "1" });
    expect(await (await latestFound()).json()).toEqual({ id: "1" });
  });
});
