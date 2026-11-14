import { describe, expect, it } from "vitest";

import { createTranslateFunction, getDictionarySync } from "@/lib/i18n/dictionaries";
import { translateField, translateOr, translateSeriesStatus } from "@/lib/i18n/optional";

const t = createTranslateFunction(getDictionarySync("fr"));

describe("translateSeriesStatus", () => {
  it("translates a known status", () => {
    expect(translateSeriesStatus(t, "ongoing")).toBe(t("seriesStatus.ongoing"));
    expect(translateSeriesStatus(t, "ongoing")).not.toBe("seriesStatus.ongoing");
  });

  it("falls back to the raw status instead of the key", () => {
    expect(translateSeriesStatus(t, "en pause éditoriale")).toBe("en pause éditoriale");
  });
});

describe("translateField", () => {
  it("falls back to the raw field name", () => {
    expect(translateField(t, "custom_field")).toBe("custom_field");
  });
});

describe("translateOr", () => {
  it("returns the fallback when t echoes the key", () => {
    const echo = (key: string) => key;
    expect(translateOr(echo as never, "x.y", "raw")).toBe("raw");
  });
});
