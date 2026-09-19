import { describe, expect, it } from "vitest";

import en from "@/lib/i18n/en";
import fr from "@/lib/i18n/fr";

function placeholders(value: string): string[] {
  return [...value.matchAll(/\{\{(\w+)\}\}/g)]
    .map((m) => m[1])
    .filter((name) => name !== "plural")
    .sort();
}

const frKeys = Object.keys(fr).sort();
const enKeys = Object.keys(en).sort();

describe("i18n dictionaries parity", () => {
  it("fr and en expose exactly the same keys", () => {
    expect(enKeys).toEqual(frKeys);
  });

  it("has no empty translation", () => {
    const empty = [...Object.entries(fr), ...Object.entries(en)]
      .filter(([, value]) => typeof value !== "string" || value.trim() === "")
      .map(([key]) => key);
    expect(empty).toEqual([]);
  });

  it("uses the same non-plural interpolation placeholders in both locales", () => {
    const mismatches = frKeys.filter((key) => {
      const frValue = (fr as Record<string, string>)[key];
      const enValue = (en as Record<string, string>)[key];
      return placeholders(enValue).join(",") !== placeholders(frValue).join(",");
    });
    expect(mismatches).toEqual([]);
  });
});
