import { describe, expect, it } from "vitest";

import type { SeriesUserActivityDto } from "@/lib/api";
import { ratedActivity, readingActivity } from "@/lib/userActivity";

function makeActivity(
  overrides: Partial<SeriesUserActivityDto> = {}
): SeriesUserActivityDto {
  return {
    user_id: "u1",
    username: "Alice",
    books_read: 0,
    books_reading: 0,
    books_total: 8,
    last_read_at: null,
    rating: null,
    ...overrides,
  };
}

describe("ratedActivity", () => {
  it("keeps only readers who rated the series", () => {
    const result = ratedActivity([
      makeActivity({ user_id: "u1", username: "Alice", rating: 8 }),
      makeActivity({ user_id: "u2", username: "Bob", rating: null }),
      makeActivity({ user_id: "u3", username: "Carol", rating: 6 }),
    ]);

    expect(result.map((item) => item.username)).toEqual(["Alice", "Carol"]);
    expect(result[0].rating).toBe(8);
  });

  it("returns an empty list when nobody rated the series", () => {
    expect(ratedActivity([makeActivity({ rating: null })])).toEqual([]);
  });
});

describe("readingActivity", () => {
  it("keeps readers who finished or started the series", () => {
    const result = readingActivity([
      makeActivity({ user_id: "u1", username: "Reader", books_read: 2 }),
      makeActivity({ user_id: "u2", username: "Curious", books_reading: 1 }),
      makeActivity({ user_id: "u3", username: "Opener", books_read: 0, books_reading: 0 }),
    ]);

    expect(result.map((item) => item.username)).toEqual(["Reader", "Curious"]);
  });

  it("ignores unread-only rows (0 read, 0 reading)", () => {
    expect(
      readingActivity([
        makeActivity({ books_read: 0, books_reading: 0, last_read_at: "2026-10-19" }),
      ])
    ).toEqual([]);
  });
});
