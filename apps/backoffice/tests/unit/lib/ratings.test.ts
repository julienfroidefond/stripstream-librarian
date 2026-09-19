import { describe, expect, it } from "vitest";

import { normalizeRating } from "@/lib/ratings";

describe("normalizeRating", () => {
  it("keeps a rating already on the 0-10 scale", () => {
    expect(normalizeRating(0, 10)).toBe(0);
    expect(normalizeRating(8, 10)).toBe(8);
    expect(normalizeRating(10, 10)).toBe(10);
  });

  it("maps a 0-5 provider scale to 0-10", () => {
    expect(normalizeRating(5, 5)).toBe(10);
    expect(normalizeRating(0, 5)).toBe(0);
    expect(normalizeRating(4.6, 5)).toBeCloseTo(9.2, 10);
  });

  it("maps a 0-100 provider scale to 0-10", () => {
    expect(normalizeRating(100, 100)).toBe(10);
    expect(normalizeRating(50, 100)).toBeCloseTo(5, 10);
    expect(normalizeRating(73, 100)).toBeCloseTo(7.3, 10);
  });
});
