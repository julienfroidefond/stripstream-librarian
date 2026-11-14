import { describe, expect, it } from "vitest";

import { safeRedirectPath } from "@/lib/auth/redirect";

describe("safeRedirectPath", () => {
  it.each([
    ["/", "/"],
    ["/series", "/series"],
    ["/series/42?page=2#top", "/series/42?page=2#top"],
  ])("keeps the relative path %s", (input, expected) => {
    expect(safeRedirectPath(input)).toBe(expected);
  });

  it.each([
    null,
    undefined,
    "",
    "series",
    "https://evil.example",
    "javascript:alert(1)",
    "//evil.example",
    "/\\evil.example",
    "/\t/evil.example",
    "/\n/evil.example",
    "/foo\\bar",
  ])("falls back to / for %j", (input) => {
    expect(safeRedirectPath(input)).toBe("/");
  });
});
