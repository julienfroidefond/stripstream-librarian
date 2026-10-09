import { describe, expect, it } from "vitest";

import {
  BOOKS_GAPS,
  SERIES_GAPS,
  gapsForTab,
  isGapTab,
  isValidGap,
  totalCountKey,
} from "@/lib/metadataGaps";

describe("isGapTab", () => {
  it("accepts the two known tabs", () => {
    expect(isGapTab("series")).toBe(true);
    expect(isGapTab("books")).toBe(true);
  });

  it("rejects anything else", () => {
    expect(isGapTab("volumes")).toBe(false);
    expect(isGapTab(undefined)).toBe(false);
    expect(isGapTab("")).toBe(false);
  });
});

describe("gapsForTab", () => {
  it("returns the series gaps for the series tab", () => {
    expect(gapsForTab("series")).toBe(SERIES_GAPS);
  });

  it("returns the books gaps for the books tab", () => {
    expect(gapsForTab("books")).toBe(BOOKS_GAPS);
  });

  it("exposes the API gap values", () => {
    expect(SERIES_GAPS.map((g) => g.value)).toEqual([
      "no_description",
      "no_genre",
      "no_authors",
      "no_publishers",
      "no_year",
      "no_cover",
      "no_community_score",
    ]);
    expect(BOOKS_GAPS.map((g) => g.value)).toEqual([
      "no_summary",
      "no_isbn",
      "no_cover",
      "no_author",
      "no_publish_date",
      "no_language",
      "no_volume",
    ]);
  });
});

describe("gap icons", () => {
  it("gives every gap a non-empty icon name", () => {
    for (const gap of [...SERIES_GAPS, ...BOOKS_GAPS]) {
      expect(gap.icon, `${gap.value} icon`).toBeTruthy();
    }
  });

  it("maps the missing-metadata concepts to distinct icons", () => {
    const iconOf = (gaps: readonly { value: string; icon: string }[], value: string) =>
      gaps.find((g) => g.value === value)?.icon;

    expect(iconOf(SERIES_GAPS, "no_description")).toBe("document");
    expect(iconOf(SERIES_GAPS, "no_genre")).toBe("tag");
    expect(iconOf(SERIES_GAPS, "no_authors")).toBe("authors");
    expect(iconOf(SERIES_GAPS, "no_publishers")).toBe("building");
    expect(iconOf(SERIES_GAPS, "no_year")).toBe("calendar");
    expect(iconOf(SERIES_GAPS, "no_cover")).toBe("image");
    expect(iconOf(SERIES_GAPS, "no_community_score")).toBe("star");
    expect(iconOf(BOOKS_GAPS, "no_isbn")).toBe("hash");
    expect(iconOf(BOOKS_GAPS, "no_language")).toBe("globe");
    expect(iconOf(BOOKS_GAPS, "no_volume")).toBe("number");
  });
});

describe("totalCountKey", () => {
  it("maps each tab to its summary total", () => {
    expect(totalCountKey("series")).toBe("series_total");
    expect(totalCountKey("books")).toBe("books_total");
  });
});

describe("isValidGap", () => {
  it("accepts a gap belonging to the tab", () => {
    expect(isValidGap("series", "no_description")).toBe(true);
    expect(isValidGap("books", "no_isbn")).toBe(true);
  });

  it("rejects a gap from the other tab", () => {
    expect(isValidGap("series", "no_isbn")).toBe(false);
    expect(isValidGap("books", "no_description")).toBe(false);
  });

  it("rejects the removed gap values", () => {
    expect(isValidGap("series", "unlinked")).toBe(false);
    expect(isValidGap("series", "missing_volumes")).toBe(false);
    expect(isValidGap("series", "stale")).toBe(false);
  });

  it("treats a missing value as invalid", () => {
    expect(isValidGap("series", undefined)).toBe(false);
    expect(isValidGap("series", "")).toBe(false);
  });

  it("rejects unknown values", () => {
    expect(isValidGap("books", "no_cover ")).toBe(false);
    expect(isValidGap("books", "cover")).toBe(false);
  });
});
