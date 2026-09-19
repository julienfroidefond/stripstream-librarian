import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const cookieStore = vi.hoisted(() => ({ get: vi.fn() }));

vi.mock("next/headers", () => ({
  cookies: async () => cookieStore,
}));

import { getServerLocale, getServerTranslations } from "@/lib/i18n/server";

beforeEach(() => {
  cookieStore.get.mockReset();
});

describe("getServerLocale", () => {
  it("returns the locale stored in the cookie", async () => {
    cookieStore.get.mockReturnValue({ value: "fr" });
    await expect(getServerLocale()).resolves.toBe("fr");
    expect(cookieStore.get).toHaveBeenCalledWith("locale");
  });

  it("falls back to the default locale for an unsupported value", async () => {
    cookieStore.get.mockReturnValue({ value: "de" });
    await expect(getServerLocale()).resolves.toBe("en");
  });

  it("falls back to the default locale without a cookie", async () => {
    cookieStore.get.mockReturnValue(undefined);
    await expect(getServerLocale()).resolves.toBe("en");
  });
});

describe("getServerTranslations", () => {
  it("returns a translate function bound to the cookie locale", async () => {
    cookieStore.get.mockReturnValue({ value: "fr" });
    const { locale, t } = await getServerTranslations();
    expect(locale).toBe("fr");
    expect(t("auth.logout")).toBe("Se déconnecter");
  });

  it("falls back to English without a cookie", async () => {
    cookieStore.get.mockReturnValue(undefined);
    const { locale, t } = await getServerTranslations();
    expect(locale).toBe("en");
    expect(t("auth.logout")).toBe("Log out");
  });
});
