import { describe, expect, it } from "vitest";

import {
  BOOK_LOCKABLE_FIELDS,
  SERIES_LOCKABLE_FIELDS,
  pickLockableFields,
} from "@/lib/metadataLocks";

describe("metadataLocks", () => {
  it("keeps only true values for known series lockable fields", () => {
    const locked = {
      description: true,
      authors: false,
      publishers: true,
      genres: true,
      cover_url: true,
    };

    expect(pickLockableFields(locked, SERIES_LOCKABLE_FIELDS)).toEqual({
      description: true,
      publishers: true,
      cover_url: true,
    });
  });

  it("drops book locks the API never reads", () => {
    const locked = {
      title: true,
      series: true,
      volume: true,
      authors: true,
      summary: true,
    };

    expect(pickLockableFields(locked, BOOK_LOCKABLE_FIELDS)).toEqual({
      authors: true,
      summary: true,
    });
  });

  it("treats missing lock maps as empty", () => {
    expect(pickLockableFields(undefined, SERIES_LOCKABLE_FIELDS)).toEqual({});
    expect(pickLockableFields(null, BOOK_LOCKABLE_FIELDS)).toEqual({});
  });
});
