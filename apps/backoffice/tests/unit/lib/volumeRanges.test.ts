import { describe, expect, it } from "vitest";

import { compressVolumes, stripLeadingArticle } from "@/lib/volumeRanges";

describe("stripLeadingArticle", () => {
  it("strips common French articles", () => {
    expect(stripLeadingArticle("L'Atelier des sorciers")).toBe("Atelier des sorciers");
    expect(stripLeadingArticle("Les Géants")).toBe("Géants");
    expect(stripLeadingArticle("Le Chat")).toBe("Chat");
    expect(stripLeadingArticle("La Maison")).toBe("Maison");
    expect(stripLeadingArticle("Une Saison")).toBe("Saison");
  });

  it("strips common English articles", () => {
    expect(stripLeadingArticle("The Hobbit")).toBe("Hobbit");
    expect(stripLeadingArticle("An Apple")).toBe("Apple");
    expect(stripLeadingArticle("A Song of Ice and Fire")).toBe("Song of Ice and Fire");
  });

  it("is case-insensitive", () => {
    expect(stripLeadingArticle("l'atelier")).toBe("atelier");
    expect(stripLeadingArticle("THE HOBBIT")).toBe("HOBBIT");
  });

  it("returns the original string when no article matches", () => {
    expect(stripLeadingArticle("Dandadan")).toBe("Dandadan");
    expect(stripLeadingArticle("Berserk")).toBe("Berserk");
    expect(stripLeadingArticle("")).toBe("");
  });

  it("does not strip a word that only looks like an article", () => {
    expect(stripLeadingArticle("Lessons in Chemistry")).toBe("Lessons in Chemistry");
    expect(stripLeadingArticle("Lego")).toBe("Lego");
  });
});

describe("compressVolumes", () => {
  it("returns an empty array for no volumes", () => {
    expect(compressVolumes([])).toEqual([]);
  });

  it("formats a single volume", () => {
    expect(compressVolumes([1])).toEqual(["T1"]);
  });

  it("compresses consecutive volumes into ranges", () => {
    expect(compressVolumes([1, 2, 3])).toEqual(["T1→3"]);
    expect(compressVolumes([7, 8, 9])).toEqual(["T7→9"]);
  });

  it("keeps isolated volumes separate", () => {
    expect(compressVolumes([1, 5, 9])).toEqual(["T1", "T5", "T9"]);
  });

  it("handles a mix of ranges and isolated volumes", () => {
    expect(compressVolumes([1, 2, 3, 5, 7, 8, 9])).toEqual(["T1→3", "T5", "T7→9"]);
  });

  it("sorts unsorted input", () => {
    expect(compressVolumes([3, 1, 2])).toEqual(["T1→3"]);
    expect(compressVolumes([9, 1, 5])).toEqual(["T1", "T5", "T9"]);
  });

  it("does not mutate the input array", () => {
    const input = [3, 1, 2];
    compressVolumes(input);
    expect(input).toEqual([3, 1, 2]);
  });
});
