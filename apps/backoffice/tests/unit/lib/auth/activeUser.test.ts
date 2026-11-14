// @vitest-environment node
import { beforeEach, describe, expect, it, vi } from "vitest";

const cookieStore = vi.hoisted(() => ({ get: vi.fn(), delete: vi.fn() }));

vi.mock("next/headers", () => ({
  cookies: async () => cookieStore,
}));

import {
  ACTIVE_USER_COOKIE,
  clearActiveUser,
  clearActiveUserIf,
  getActiveUser,
  isStaleActiveUserError,
} from "@/lib/auth/activeUser";

beforeEach(() => {
  cookieStore.get.mockReset();
  cookieStore.delete.mockReset();
});

describe("isStaleActiveUserError", () => {
  it("matches the API rejections of X-As-User", () => {
    expect(isStaleActiveUserError(400, "X-As-User references an unknown user")).toBe(true);
    expect(isStaleActiveUserError(400, "X-As-User must be a valid user id")).toBe(true);
  });

  it("ignores other errors", () => {
    expect(isStaleActiveUserError(400, "bad input")).toBe(false);
    expect(isStaleActiveUserError(404, "X-As-User references an unknown user")).toBe(false);
    expect(isStaleActiveUserError(400, undefined)).toBe(false);
  });
});

describe("clearActiveUser / clearActiveUserIf", () => {
  it("deletes the cookie", async () => {
    await clearActiveUser();
    expect(cookieStore.delete).toHaveBeenCalledWith(ACTIVE_USER_COOKIE);
  });

  it("swallows a read-only cookie store", async () => {
    cookieStore.delete.mockImplementation(() => {
      throw new Error("Cookies can only be modified in a Server Action");
    });
    await expect(clearActiveUser()).resolves.toBeUndefined();
  });

  it("only clears the selection of the given reader", async () => {
    cookieStore.get.mockReturnValue({ value: "u1" });
    await expect(clearActiveUserIf("u2")).resolves.toBe(false);
    expect(cookieStore.delete).not.toHaveBeenCalled();
    await expect(clearActiveUserIf("u1")).resolves.toBe(true);
    expect(cookieStore.delete).toHaveBeenCalledWith(ACTIVE_USER_COOKIE);
  });
});

describe("getActiveUser", () => {
  it("returns the selected reader when the cookie is set", async () => {
    cookieStore.get.mockReturnValue({ value: "user-42" });

    await expect(getActiveUser()).resolves.toEqual({
      userId: "user-42",
      hasActiveUser: true,
    });
    expect(cookieStore.get).toHaveBeenCalledWith(ACTIVE_USER_COOKIE);
  });

  it("returns admin mode without the cookie", async () => {
    cookieStore.get.mockReturnValue(undefined);

    await expect(getActiveUser()).resolves.toEqual({
      userId: null,
      hasActiveUser: false,
    });
  });

  it("treats an empty cookie value as admin mode", async () => {
    cookieStore.get.mockReturnValue({ value: "" });

    await expect(getActiveUser()).resolves.toEqual({
      userId: null,
      hasActiveUser: false,
    });
  });
});
