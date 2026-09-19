// @vitest-environment node
import { describe, expect, it, vi } from "vitest";

import { jsonRequest, routeCtx } from "./helpers";

vi.mock("@/lib/api", async () => {
  const { createApiMock } = await import("./helpers");
  return createApiMock();
});

import { apiFetch } from "@/lib/api";
import { GET, POST, DELETE } from "@/app/api/telegram-monitor/[...path]/route";

const mockApi = vi.mocked(apiFetch);

describe("/api/telegram-monitor/[...path]", () => {
  it("GET joins the path segments and forwards the query string", async () => {
    mockApi.mockResolvedValue({ items: [] });
    const res = await GET(
      jsonRequest("/api/telegram-monitor/channels/list?limit=10"),
      routeCtx({ path: ["channels", "list"] })
    );
    expect(mockApi).toHaveBeenCalledWith("/telegram-monitor/channels/list?limit=10", {
      method: "GET",
      body: undefined,
    });
    expect(res.status).toBe(200);
  });

  it("POST parses and forwards the JSON body", async () => {
    mockApi.mockResolvedValue({ ok: true });
    const res = await POST(
      jsonRequest("/api/telegram-monitor/download", { method: "POST", body: { id: 1 } }),
      routeCtx({ path: ["download"] })
    );
    expect(mockApi).toHaveBeenCalledWith("/telegram-monitor/download", {
      method: "POST",
      body: JSON.stringify({ id: 1 }),
    });
    expect(res.status).toBe(200);
  });

  it("POST with an empty body forwards undefined", async () => {
    mockApi.mockResolvedValue(undefined);
    await POST(jsonRequest("/api/telegram-monitor/ping", { method: "POST" }), routeCtx({ path: ["ping"] }));
    expect(mockApi).toHaveBeenCalledWith("/telegram-monitor/ping", {
      method: "POST",
      body: undefined,
    });
  });

  it("DELETE does not parse a body", async () => {
    mockApi.mockResolvedValue({ deleted: true });
    await DELETE(
      jsonRequest("/api/telegram-monitor/items/1", { method: "DELETE" }),
      routeCtx({ path: ["items", "1"] })
    );
    expect(mockApi).toHaveBeenCalledWith("/telegram-monitor/items/1", {
      method: "DELETE",
      body: undefined,
    });
  });

  it("maps a 404 error message to status 404", async () => {
    mockApi.mockRejectedValue(new Error("upstream (404)"));
    const res = await GET(jsonRequest("/api/telegram-monitor/x"), routeCtx({ path: ["x"] }));
    expect(res.status).toBe(404);
  });

  it("maps a 400 error message to status 400", async () => {
    mockApi.mockRejectedValue(new Error("upstream (400)"));
    const res = await GET(jsonRequest("/api/telegram-monitor/x"), routeCtx({ path: ["x"] }));
    expect(res.status).toBe(400);
  });

  it("maps other errors to 500", async () => {
    mockApi.mockRejectedValue(new Error("boom"));
    const res = await GET(jsonRequest("/api/telegram-monitor/x"), routeCtx({ path: ["x"] }));
    expect(res.status).toBe(500);
  });

  it("returns ok when the upstream body is empty", async () => {
    mockApi.mockResolvedValue(undefined);
    const res = await GET(jsonRequest("/api/telegram-monitor/x"), routeCtx({ path: ["x"] }));
    expect(await res.json()).toEqual({ ok: true });
  });
});
