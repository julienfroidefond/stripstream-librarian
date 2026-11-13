// @vitest-environment node
import { beforeEach, describe, expect, it, vi } from "vitest";

const revalidateTag = vi.hoisted(() => vi.fn());
const revalidatePath = vi.hoisted(() => vi.fn());
vi.mock("next/cache", () => ({ revalidateTag, revalidatePath }));

import { cacheTags, invalidateAfter, invalidationPlan, type DomainWrite } from "@/lib/cache";

type Row = { write: DomainWrite; tags: string[]; paths: string[] };

/** Event → exact tags and paths. Paths of `layout` type are written `path@layout`. */
const TABLE: Row[] = [
  {
    write: { type: "series.updated", seriesId: "s1" },
    tags: ["series", "series:s1"],
    paths: ["/series", "/series/s1"],
  },
  {
    write: { type: "series.deleted", seriesId: "s1" },
    tags: ["series", "series:s1", "books", "metadata-gaps", "reading-lists", "stats"],
    paths: ["/series", "/series/s1", "/books", "/reading-lists"],
  },
  {
    write: { type: "series.merged", targetId: "t", sourceId: "s" },
    tags: ["series", "series:t", "series:s", "books", "metadata", "metadata-gaps", "reading-lists"],
    paths: ["/series", "/series/t", "/series/s", "/books", "/reading-lists"],
  },
  {
    write: { type: "series.merged", targetId: "t" },
    tags: ["series", "series:t", "books", "metadata", "metadata-gaps", "reading-lists"],
    paths: ["/series", "/series/t", "/books", "/reading-lists"],
  },
  {
    write: { type: "series.created" },
    tags: ["series", "metadata-gaps"],
    paths: ["/series", "/libraries"],
  },
  {
    write: { type: "series.booksRenamed", seriesId: "s1" },
    tags: ["series", "series:s1", "books"],
    paths: ["/series", "/series/s1", "/books"],
  },
  {
    write: { type: "series.orphansDeleted" },
    tags: ["series", "metadata-gaps"],
    paths: ["/series", "/settings"],
  },
  {
    write: { type: "book.updated", bookId: "b1", seriesId: "s1" },
    tags: ["books", "series", "series:s1"],
    paths: ["/books", "/books/b1", "/series", "/series/s1"],
  },
  {
    write: { type: "book.updated", bookId: "b1", seriesId: null },
    tags: ["books", "series"],
    paths: ["/books", "/books/b1", "/series"],
  },
  {
    write: { type: "book.deleted", bookId: "b1", seriesId: "s1" },
    tags: ["books", "series", "series:s1", "stats"],
    paths: ["/books", "/series", "/series/s1"],
  },
  {
    write: { type: "readingProgress.changed", bookId: "b1", seriesId: "s1" },
    tags: ["stats", "books", "series", "series:s1"],
    paths: ["/", "/books/b1", "/series", "/series/s1"],
  },
  {
    write: { type: "readingProgress.changed", seriesId: null },
    tags: ["stats", "books", "series"],
    paths: ["/", "/series"],
  },
  {
    write: { type: "genres.changed" },
    tags: ["series", "metadata-gaps"],
    paths: ["/genres", "/series"],
  },
  {
    write: { type: "genres.changed", seriesIds: ["a", "b"] },
    tags: ["series", "series:a", "series:b", "metadata-gaps"],
    paths: ["/genres", "/series"],
  },
  { write: { type: "library.created" }, tags: [], paths: ["/libraries"] },
  {
    write: { type: "library.deleted", libraryId: "l1" },
    tags: ["series", "books", "stats"],
    paths: ["/libraries", "/libraries/l1", "/series", "/books"],
  },
  { write: { type: "library.updated", libraryId: "l1" }, tags: [], paths: ["/libraries", "/libraries/l1"] },
  {
    write: { type: "readingLists.changed" },
    tags: ["reading-lists"],
    paths: ["/series", "/reading-lists"],
  },
  { write: { type: "metadata.changed" }, tags: ["metadata", "series"], paths: [] },
  {
    write: { type: "metadata.changed", seriesId: "s1", linkId: "k1" },
    tags: ["metadata", "series", "series:s1", "link:k1"],
    paths: [],
  },
  { write: { type: "jobs.started" }, tags: [], paths: ["/jobs"] },
  { write: { type: "jobs.finished" }, tags: ["books", "series", "stats"], paths: [] },
  { write: { type: "activeUser.changed" }, tags: [], paths: ["/@layout"] },
  { write: { type: "catalog.refreshRequested", scope: "books" }, tags: ["books"], paths: [] },
  { write: { type: "catalog.refreshRequested", scope: "series" }, tags: ["series"], paths: [] },
  {
    write: { type: "catalog.refreshRequested", scope: "series-detail", seriesId: "s1" },
    tags: ["series:s1"],
    paths: [],
  },
];

describe("invalidationPlan", () => {
  it.each(TABLE.map((row) => [JSON.stringify(row.write), row] as const))("%s", (_name, row) => {
    const plan = invalidationPlan(row.write);
    expect(plan.tags).toEqual(row.tags);
    expect(plan.paths.map(({ path, type }) => (type ? `${path}@${type}` : path))).toEqual(row.paths);
  });
});

describe("invalidateAfter", () => {
  beforeEach(() => {
    revalidateTag.mockClear();
    revalidatePath.mockClear();
  });

  it("expires every tag immediately and revalidates every path", () => {
    invalidateAfter({ type: "book.deleted", bookId: "b1", seriesId: "s1" });
    expect(revalidateTag.mock.calls).toEqual([
      ["books", { expire: 0 }],
      ["series", { expire: 0 }],
      ["series:s1", { expire: 0 }],
      ["stats", { expire: 0 }],
    ]);
    expect(revalidatePath.mock.calls).toEqual([["/books"], ["/series"], ["/series/s1"]]);
  });

  it("deduplicates tags", () => {
    invalidateAfter({ type: "genres.changed", seriesIds: ["a", "a"] });
    expect(revalidateTag.mock.calls.map(([tag]) => tag)).toEqual(["series", "series:a", "metadata-gaps"]);
  });

  it("passes the layout type through", () => {
    invalidateAfter({ type: "activeUser.changed" });
    expect(revalidatePath).toHaveBeenCalledWith("/", "layout");
  });
});

describe("cacheTags", () => {
  it("builds the tag strings shared by readers and writes", () => {
    expect(cacheTags.books()).toBe("books");
    expect(cacheTags.series()).toBe("series");
    expect(cacheTags.seriesById("x")).toBe("series:x");
    expect(cacheTags.metadataGaps()).toBe("metadata-gaps");
    expect(cacheTags.metadata()).toBe("metadata");
    expect(cacheTags.metadataLink("k")).toBe("link:k");
    expect(cacheTags.readingLists()).toBe("reading-lists");
    expect(cacheTags.stats()).toBe("stats");
  });
});
