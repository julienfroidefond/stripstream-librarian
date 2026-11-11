import { describe, expect, it } from "vitest";

import { catalogQueryParams, isCatalogVolumeType, type CatalogQuery } from "@/lib/api";

describe("catalogQueryParams", () => {
  it("defaults paging and omits unset filters", () => {
    expect(catalogQueryParams({}).toString()).toBe("page=1&limit=50");
  });

  it("encodes every catalog filter under its wire name", () => {
    const query: CatalogQuery = {
      libraryId: "lib-1",
      search: "Urasawa & Naoki",
      readingStatus: "unread,reading",
      sort: "latest",
      seriesStatus: "ongoing",
      hasMissing: true,
      metadataProvider: "google_books",
      author: "Naoki Urasawa",
      shelf: "wishlist",
      volumeType: "regular",
      rating: "rated",
      gap: "no_description",
      page: 2,
      limit: 48,
    };

    expect(catalogQueryParams(query).toString()).toBe(
      "library_id=lib-1&q=Urasawa+%26+Naoki&reading_status=unread%2Creading&sort=latest&series_status=ongoing&has_missing=true&metadata_provider=google_books&author=Naoki+Urasawa&no_books=true&volume_type=regular&rated_only=true&gap=no_description&page=2&limit=48"
    );
  });

  it("maps the in-library shelf to has_books", () => {
    expect(catalogQueryParams({ shelf: "in_library" }).toString()).toBe(
      "has_books=true&page=1&limit=50"
    );
  });

  it("maps the unrated filter to rated_only=unrated", () => {
    expect(catalogQueryParams({ rating: "unrated" }).toString()).toBe(
      "rated_only=unrated&page=1&limit=50"
    );
  });

  it("omits false booleans and empty strings", () => {
    expect(catalogQueryParams({ hasMissing: false, search: "", sort: "" }).toString()).toBe(
      "page=1&limit=50"
    );
  });
});

describe("isCatalogVolumeType", () => {
  it("accepts the API volume types", () => {
    for (const value of ["regular", "hs", "oneshot", "integral"]) {
      expect(isCatalogVolumeType(value)).toBe(true);
    }
  });

  it("rejects unknown or empty values", () => {
    expect(isCatalogVolumeType("volume")).toBe(false);
    expect(isCatalogVolumeType("")).toBe(false);
    expect(isCatalogVolumeType(undefined)).toBe(false);
  });
});
