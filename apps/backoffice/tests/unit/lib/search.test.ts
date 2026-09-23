import { describe, it, expect } from "vitest";

import { matchesSearchText, normalizeSearchText } from "@/lib/search";

describe("normalizeSearchText", () => {
  it("lowercases and strips diacritics", () => {
    expect(normalizeSearchText("Éditions Astérix")).toBe("editions asterix");
  });

  it("trims surrounding whitespace", () => {
    expect(normalizeSearchText("  Bételgeuse  ")).toBe("betelgeuse");
  });

  it("leaves already-normalized text unchanged", () => {
    expect(normalizeSearchText("obélix")).toBe("obelix");
    expect(normalizeSearchText("plain")).toBe("plain");
  });

  it("returns an empty string for blank input", () => {
    expect(normalizeSearchText("   ")).toBe("");
  });
});

describe("matchesSearchText", () => {
  it("matches accented values from an unaccented query", () => {
    expect(matchesSearchText("Astérix", "asterix")).toBe(true);
  });

  it("matches unaccented values from an accented query", () => {
    expect(matchesSearchText("Asterix", "astérix")).toBe(true);
  });

  it("is case-insensitive", () => {
    expect(matchesSearchText("TINTIN", "tintin")).toBe(true);
  });

  it("matches on partial substrings", () => {
    expect(matchesSearchText("Le Petit Prince", "petit")).toBe(true);
  });

  it("treats an empty query as matching everything", () => {
    expect(matchesSearchText("Anything", "")).toBe(true);
    expect(matchesSearchText("Anything", "   ")).toBe(true);
  });

  it("returns false when the value does not contain the query", () => {
    expect(matchesSearchText("Astérix", "tintin")).toBe(false);
  });
});
