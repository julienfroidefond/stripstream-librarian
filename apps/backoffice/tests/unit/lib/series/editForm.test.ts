import { describe, expect, it } from "vitest";

import { seriesEditDefaults, type EditSeriesModel } from "@/lib/series/editForm";

const model: EditSeriesModel = {
  seriesId: "s1",
  seriesName: "Berserk",
  editBookAuthor: "Kentaro Miura",
  editBookLanguage: null,
  metadata: {
    series_name: "Berserk",
    authors: ["Kentaro Miura"],
    genres: ["Seinen"],
    publishers: [],
    description: null,
    start_year: 1989,
    total_volumes: null,
    status: "ongoing",
    book_author: null,
    book_language: null,
    locked_fields: { start_year: true },
  },
};

describe("seriesEditDefaults", () => {
  it("maps the series model to form values", () => {
    expect(seriesEditDefaults(model)).toEqual({
      name: "Berserk",
      authors: ["Kentaro Miura"],
      genres: ["Seinen"],
      publishers: [],
      description: "",
      startYear: "1989",
      totalVolumes: "",
      status: "ongoing",
      lockedFields: { start_year: true },
      bookAuthor: "Kentaro Miura",
      bookLanguage: "",
    });
  });

  it("starts empty without metadata and for the unclassified series", () => {
    expect(
      seriesEditDefaults({ ...model, seriesName: "unclassified", metadata: null, editBookAuthor: null }),
    ).toEqual({
      name: "",
      authors: [],
      genres: [],
      publishers: [],
      description: "",
      startYear: "",
      totalVolumes: "",
      status: "",
      lockedFields: {},
      bookAuthor: "",
      bookLanguage: "",
    });
  });
});
