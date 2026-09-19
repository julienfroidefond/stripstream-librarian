// @vitest-environment node
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const cookieStore = vi.hoisted(() => ({ get: vi.fn() }));

vi.mock("next/headers", () => ({
  cookies: async () => cookieStore,
}));

import {
  SESSION_COOKIE,
  createSessionToken,
  getSession,
  verifySessionToken,
} from "@/lib/session";

const SECRET = "unit-test-secret-value-at-least-32-bytes";
let originalSecret: string | undefined;

beforeEach(() => {
  originalSecret = process.env.SESSION_SECRET;
  process.env.SESSION_SECRET = SECRET;
  cookieStore.get.mockReset();
});

afterEach(() => {
  if (originalSecret === undefined) {
    delete process.env.SESSION_SECRET;
  } else {
    process.env.SESSION_SECRET = originalSecret;
  }
});

describe("createSessionToken / verifySessionToken", () => {
  it("creates a verifiable JWT with three segments", async () => {
    const token = await createSessionToken();
    expect(token.split(".")).toHaveLength(3);
    await expect(verifySessionToken(token)).resolves.toBe(true);
  });

  it("rejects a tampered token", async () => {
    const token = await createSessionToken();
    await expect(verifySessionToken(`${token}x`)).resolves.toBe(false);
    await expect(verifySessionToken("not-a-jwt")).resolves.toBe(false);
  });

  it("rejects a token signed with another secret", async () => {
    const token = await createSessionToken();
    process.env.SESSION_SECRET = "another-secret-value-at-least-32-bytes";
    await expect(verifySessionToken(token)).resolves.toBe(false);
  });

  it("fails when SESSION_SECRET is missing", async () => {
    delete process.env.SESSION_SECRET;
    await expect(createSessionToken()).rejects.toThrow("SESSION_SECRET");
    await expect(verifySessionToken("anything")).resolves.toBe(false);
  });
});

describe("getSession", () => {
  it("returns false without a session cookie", async () => {
    cookieStore.get.mockReturnValue(undefined);
    await expect(getSession()).resolves.toBe(false);
    expect(cookieStore.get).toHaveBeenCalledWith(SESSION_COOKIE);
  });

  it("returns false for an invalid cookie value", async () => {
    cookieStore.get.mockReturnValue({ value: "garbage" });
    await expect(getSession()).resolves.toBe(false);
  });

  it("returns true for a valid session cookie", async () => {
    const token = await createSessionToken();
    cookieStore.get.mockReturnValue({ value: token });
    await expect(getSession()).resolves.toBe(true);
  });
});
