// @vitest-environment node
import { beforeEach, describe, expect, it, vi } from "vitest";

const cookieStore = vi.hoisted(() => ({ get: vi.fn() }));

vi.mock("next/headers", () => ({
  cookies: async () => cookieStore,
}));

import { ACTIVE_USER_COOKIE, getActiveUser } from "@/lib/activeUser";

beforeEach(() => {
  cookieStore.get.mockReset();
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
