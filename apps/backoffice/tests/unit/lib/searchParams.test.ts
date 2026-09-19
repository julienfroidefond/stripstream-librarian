import { describe, expect, it } from "vitest";

import { paramBool, paramInt, paramString, paramStringOr } from "@/lib/searchParams";

describe("paramString", () => {
  it("returns a non-empty string value", () => {
    expect(paramString({ q: "dandadan" }, "q")).toBe("dandadan");
  });

  it("returns undefined for missing, empty, or non-string values", () => {
    expect(paramString({}, "q")).toBeUndefined();
    expect(paramString({ q: "" }, "q")).toBeUndefined();
    expect(paramString({ q: ["a", "b"] }, "q")).toBeUndefined();
    expect(paramString({ q: undefined }, "q")).toBeUndefined();
  });
});

describe("paramStringOr", () => {
  it("returns the value when present", () => {
    expect(paramStringOr({ sort: "title" }, "sort", "date")).toBe("title");
  });

  it("returns the default for missing, empty, or array values", () => {
    expect(paramStringOr({}, "sort", "date")).toBe("date");
    expect(paramStringOr({ sort: "" }, "sort", "date")).toBe("date");
    expect(paramStringOr({ sort: ["a"] }, "sort", "date")).toBe("date");
  });
});

describe("paramInt", () => {
  it("parses integer values", () => {
    expect(paramInt({ page: "12" }, "page", 1)).toBe(12);
    expect(paramInt({ page: "-3" }, "page", 1)).toBe(-3);
  });

  it("truncates decimal values", () => {
    expect(paramInt({ page: "3.9" }, "page", 1)).toBe(3);
  });

  it("parses a leading number and ignores the rest", () => {
    expect(paramInt({ page: "12abc" }, "page", 1)).toBe(12);
  });

  it("falls back to the default for missing or unparseable values", () => {
    expect(paramInt({}, "page", 1)).toBe(1);
    expect(paramInt({ page: "abc" }, "page", 1)).toBe(1);
    expect(paramInt({ page: "" }, "page", 1)).toBe(1);
    expect(paramInt({ page: ["1", "2"] }, "page", 1)).toBe(1);
  });
});

describe("paramBool", () => {
  it("is true only for the exact string \"true\"", () => {
    expect(paramBool({ full: "true" }, "full")).toBe(true);
  });

  it("is false for anything else", () => {
    expect(paramBool({ full: "false" }, "full")).toBe(false);
    expect(paramBool({ full: "1" }, "full")).toBe(false);
    expect(paramBool({ full: "TRUE" }, "full")).toBe(false);
    expect(paramBool({ full: ["true"] }, "full")).toBe(false);
    expect(paramBool({}, "full")).toBe(false);
  });
});
