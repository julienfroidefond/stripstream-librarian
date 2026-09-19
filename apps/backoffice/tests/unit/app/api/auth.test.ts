// @vitest-environment node
import { beforeEach, describe, expect, it, vi } from "vitest";

import { jsonRequest } from "./helpers";

const session = vi.hoisted(() => ({
  createSessionToken: vi.fn(),
  SESSION_COOKIE: "sl_session",
}));

vi.mock("@/lib/session", () => session);

import { POST as login } from "@/app/api/auth/login/route";
import { POST as logout } from "@/app/api/auth/logout/route";

const originalEnv = { ...process.env };

describe("POST /api/auth/login", () => {
  beforeEach(() => {
    process.env = { ...originalEnv };
    process.env.ADMIN_USERNAME = "admin";
    process.env.ADMIN_PASSWORD = "secret";
    session.createSessionToken.mockResolvedValue("signed-token");
  });

  it("rejects a malformed body with 400", async () => {
    const res = await login(jsonRequest("/api/auth/login", { method: "POST", body: { username: 1 } }));
    expect(res.status).toBe(400);
    expect(await res.json()).toEqual({ error: "Invalid request" });
    expect(session.createSessionToken).not.toHaveBeenCalled();
  });

  it("rejects an invalid JSON body with 400", async () => {
    const req = new Request("http://localhost:7082/api/auth/login", {
      method: "POST",
      body: "not-json",
    });
    const res = await login(req as never);
    expect(res.status).toBe(400);
  });

  it("returns 500 when ADMIN_PASSWORD is unset", async () => {
    delete process.env.ADMIN_PASSWORD;
    const res = await login(
      jsonRequest("/api/auth/login", { method: "POST", body: { username: "admin", password: "x" } })
    );
    expect(res.status).toBe(500);
    expect(await res.json()).toEqual({ error: "Server misconfiguration" });
  });

  it("returns 401 on wrong credentials", async () => {
    const res = await login(
      jsonRequest("/api/auth/login", { method: "POST", body: { username: "admin", password: "nope" } })
    );
    expect(res.status).toBe(401);
    expect(await res.json()).toEqual({ error: "Invalid credentials" });
  });

  it("uses the default admin username when ADMIN_USERNAME is unset", async () => {
    delete process.env.ADMIN_USERNAME;
    const res = await login(
      jsonRequest("/api/auth/login", { method: "POST", body: { username: "admin", password: "secret" } })
    );
    expect(res.status).toBe(200);
  });

  it("sets the session cookie on success", async () => {
    const res = await login(
      jsonRequest("/api/auth/login", { method: "POST", body: { username: "admin", password: "secret" } })
    );
    expect(res.status).toBe(200);
    expect(await res.json()).toEqual({ success: true });
    const cookie = res.cookies.get("sl_session");
    expect(cookie?.value).toBe("signed-token");
    expect(cookie?.httpOnly).toBe(true);
    expect(cookie?.sameSite).toBe("lax");
    expect(cookie?.maxAge).toBe(7 * 24 * 60 * 60);
  });
});

describe("POST /api/auth/logout", () => {
  it("clears the session cookie", async () => {
    const res = await logout();
    expect(res.status).toBe(200);
    expect(await res.json()).toEqual({ success: true });
    const cookie = res.cookies.get("sl_session");
    expect(cookie?.value).toBe("");
  });
});
