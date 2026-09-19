import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.unmock("@/lib/i18n/context");

import { LocaleProvider, useTranslation } from "@/lib/i18n/context";
import type { ReactNode } from "react";

function wrapperFor(initialLocale: "fr" | "en") {
  return function Wrapper({ children }: { children: ReactNode }) {
    return <LocaleProvider initialLocale={initialLocale}>{children}</LocaleProvider>;
  };
}

let fetchMock: ReturnType<typeof vi.fn>;

beforeEach(() => {
  document.cookie = "locale=;max-age=0;path=/";
  Object.defineProperty(window, "location", {
    configurable: true,
    value: { href: "", reload: vi.fn(), assign: vi.fn() },
  });
  fetchMock = vi.fn().mockResolvedValue({ ok: true, json: async () => ({}) });
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("useTranslation", () => {
  it("throws when used outside a LocaleProvider", () => {
    expect(() => renderHook(() => useTranslation())).toThrow(
      "useTranslation must be used within a LocaleProvider"
    );
  });

  it("exposes the initial locale and its dictionary", () => {
    const { result } = renderHook(() => useTranslation(), {
      wrapper: wrapperFor("en"),
    });

    expect(result.current.locale).toBe("en");
    expect(result.current.t("nav.tokens")).toBe("Tokens");
  });

  it("loads the other locale", () => {
    const { result } = renderHook(() => useTranslation(), {
      wrapper: wrapperFor("fr"),
    });

    expect(result.current.locale).toBe("fr");
    expect(result.current.t("nav.tokens")).toBe("Jetons");
  });
});

describe("setLocale", () => {
  it("persists the locale in a cookie, saves it and reloads", async () => {
    const { result } = renderHook(() => useTranslation(), {
      wrapper: wrapperFor("en"),
    });

    await act(async () => {
      await result.current.setLocale("fr");
    });

    expect(document.cookie).toContain("locale=fr");

    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toMatch(/\/settings$/);
    expect(init.method).toBe("PATCH");
    expect(JSON.parse(init.body as string)).toEqual({ language: "fr" });
    expect(window.location.reload).toHaveBeenCalledTimes(1);
  });

  it("still reloads when saving the locale fails", async () => {
    fetchMock.mockRejectedValue(new Error("network down"));

    const { result } = renderHook(() => useTranslation(), {
      wrapper: wrapperFor("en"),
    });

    await act(async () => {
      await result.current.setLocale("fr");
    });

    expect(document.cookie).toContain("locale=fr");
    expect(window.location.reload).toHaveBeenCalledTimes(1);
  });
});
