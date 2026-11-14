// @vitest-environment node
import { SignJWT } from "jose";
import { NextRequest } from "next/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("next/headers", () => ({ cookies: async () => ({ get: () => undefined }) }));

import { SESSION_COOKIE } from "@/lib/auth/session";
import { proxy } from "@/proxy";

const SECRET = "proxy-test-secret-value-at-least-32-bytes";
let original: string | undefined;

async function token(secret = SECRET) {
  return new SignJWT({})
    .setProtectedHeader({ alg: "HS256" })
    .setExpirationTime("1h")
    .sign(new TextEncoder().encode(secret));
}

function request(path: string, sessionToken?: string) {
  const req = new NextRequest(new URL(path, "http://localhost:7082"));
  if (sessionToken) req.cookies.set(SESSION_COOKIE, sessionToken);
  return req;
}

beforeEach(() => {
  original = process.env.SESSION_SECRET;
  process.env.SESSION_SECRET = SECRET;
  vi.spyOn(console, "error").mockImplementation(() => {});
});

afterEach(() => {
  if (original === undefined) delete process.env.SESSION_SECRET;
  else process.env.SESSION_SECRET = original;
  vi.restoreAllMocks();
});

describe("proxy", () => {
  it("lets a valid session through", async () => {
    const res = await proxy(request("/jobs", await token()));
    expect(res.headers.get("x-middleware-next")).toBe("1");
  });

  it("redirects pages to /login without a session", async () => {
    const res = await proxy(request("/jobs"));
    expect(res.status).toBe(307);
    expect(res.headers.get("location")).toBe("http://localhost:7082/login?from=%2Fjobs");
  });

  it.each([undefined, "too-short"])(
    "fails closed when SESSION_SECRET is %j (page → /login)",
    async (secret) => {
      // Token signed with the insecure value the old fallback accepted.
      const forged = await token("dev-insecure-secret");
      if (secret === undefined) delete process.env.SESSION_SECRET;
      else process.env.SESSION_SECRET = secret;

      const res = await proxy(request("/jobs", forged));
      expect(res.status).toBe(307);
      expect(res.headers.get("location")).toContain("/login");
    },
  );

  it("fails closed with 401 on API routes when the secret is unusable", async () => {
    process.env.SESSION_SECRET = "short";
    const res = await proxy(request("/api/jobs", await token()));
    expect(res.status).toBe(401);
    await expect(res.json()).resolves.toEqual({ error: "Unauthorized" });
  });

  it("still serves /login when the secret is unusable", async () => {
    delete process.env.SESSION_SECRET;
    const res = await proxy(request("/login"));
    expect(res.headers.get("x-middleware-next")).toBe("1");
  });
});
