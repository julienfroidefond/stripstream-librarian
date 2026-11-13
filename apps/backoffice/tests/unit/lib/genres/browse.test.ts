// @vitest-environment node
import { afterEach, describe, expect, it, vi } from "vitest";

import { genresApi, genreBrowsePath } from "@/lib/api/browser/genres";
import {
  ALL_SERIES,
  GENRES_PAGE_SIZE,
  UNTAGGED_SERIES,
  browseQuery,
  isAllSeries,
  removeGenreFromSelection,
  renameGenreInSelection,
  toggleGenre,
} from "@/lib/genres/browse";

import { exportedMethods, resolveProxyRoute } from "../../helpers/proxyRoutes";

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("genreBrowsePath", () => {
  it("repeats array keys and omits empty filters", () => {
    expect(
      genreBrowsePath("/api/genres/browse", {
        genres: ["Action", "Science-fiction & co"],
        exclude: [],
        libraryIds: ["l1", "l2"],
        untagged: false,
        q: "  ",
        page: 2,
        limit: 50,
      })
    ).toBe(
      "/api/genres/browse?genres=Action&genres=Science-fiction%20%26%20co&library_ids=l1&library_ids=l2&page=2&limit=50"
    );
  });

  it("sends untagged only when set", () => {
    expect(genreBrowsePath("/genres/browse", { untagged: true, q: " drag " })).toBe(
      "/genres/browse?untagged=true&q=drag"
    );
    expect(genreBrowsePath("/genres/browse", {})).toBe("/genres/browse");
  });
});

describe("genresApi.browse", () => {
  it("makes a single GET on an existing proxy route", async () => {
    const page = { items: [], total: 0, page: 1, limit: 100, facets: { genres: [], untagged: 0, total: 0 } };
    const fetchMock = vi.fn(async () => new Response(JSON.stringify(page), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    }));
    vi.stubGlobal("fetch", fetchMock);

    await expect(genresApi.browse({ genres: ["Action"], exclude: ["Horror"] })).resolves.toEqual(page);

    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [url, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe("/api/genres/browse?genres=Action&exclude=Horror");
    expect(init.method).toBe("GET");
    const file = resolveProxyRoute(url);
    expect(file).not.toBeNull();
    expect(exportedMethods(file!)).toContain("GET");
  });
});

describe("browseQuery", () => {
  it("sends only untagged for the untagged view", () => {
    expect(browseQuery({ untagged: true, genres: ["A"], exclude: ["B"] }, ["l1"], " x ")).toEqual({
      untagged: true,
      libraryIds: ["l1"],
      q: "x",
      page: 1,
      limit: GENRES_PAGE_SIZE,
    });
  });

  it("sends genres and exclusions otherwise", () => {
    expect(browseQuery({ untagged: false, genres: ["A"], exclude: ["B"] }, [], "", 3)).toEqual({
      genres: ["A"],
      exclude: ["B"],
      libraryIds: [],
      q: undefined,
      page: 3,
      limit: GENRES_PAGE_SIZE,
    });
  });
});

describe("selection transitions", () => {
  it("leaves the untagged view when a genre is toggled", () => {
    expect(toggleGenre(UNTAGGED_SERIES, "A", "include")).toEqual({ untagged: false, genres: ["A"], exclude: [] });
    expect(toggleGenre(UNTAGGED_SERIES, "A", "exclude")).toEqual({ untagged: false, genres: [], exclude: ["A"] });
  });

  it("toggles and moves a genre between include and exclude", () => {
    const included = toggleGenre(ALL_SERIES, "A", "include");
    expect(toggleGenre(included, "A", "include")).toEqual(ALL_SERIES);
    expect(toggleGenre(included, "A", "exclude")).toEqual({ untagged: false, genres: [], exclude: ["A"] });
  });

  it("follows a rename without duplicating", () => {
    expect(renameGenreInSelection({ untagged: false, genres: ["A", "B"], exclude: ["A"] }, "A", "B")).toEqual({
      untagged: false,
      genres: ["B"],
      exclude: ["B"],
    });
  });

  it("drops a deleted genre", () => {
    expect(removeGenreFromSelection({ untagged: false, genres: ["A", "B"], exclude: ["A"] }, "A")).toEqual({
      untagged: false,
      genres: ["B"],
      exclude: [],
    });
    expect(isAllSeries(removeGenreFromSelection({ untagged: false, genres: ["A"], exclude: [] }, "A"))).toBe(true);
  });
});
