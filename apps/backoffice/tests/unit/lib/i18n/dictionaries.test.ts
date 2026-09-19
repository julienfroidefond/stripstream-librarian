import { describe, expect, it } from "vitest";

import { createTranslateFunction, getDictionarySync } from "@/lib/i18n/dictionaries";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";

type Dict = Parameters<typeof createTranslateFunction>[0];

function translate(dict: Record<string, string>) {
  return createTranslateFunction(dict as Dict) as TranslateFunction;
}

describe("createTranslateFunction", () => {
  it("returns the value for a known key", () => {
    const t = translate({ "auth.logout": "Log out" });
    expect(t("auth.logout" as never)).toBe("Log out");
  });

  it("falls back to the key itself when it is missing", () => {
    const t = translate({});
    expect(t("missing.key" as never)).toBe("missing.key");
  });

  it("leaves placeholders untouched without params", () => {
    const t = translate({ greet: "Hello {{name}}" });
    expect(t("greet" as never)).toBe("Hello {{name}}");
  });

  it("interpolates a single placeholder", () => {
    const t = translate({ greet: "Hello {{name}}" });
    expect(t("greet" as never, { name: "Ada" })).toBe("Hello Ada");
  });

  it("interpolates multiple placeholders", () => {
    const t = translate({ range: "{{start}} to {{end}}" });
    expect(t("range" as never, { start: 1, end: 9 })).toBe("1 to 9");
  });

  it("replaces every occurrence of a repeated placeholder", () => {
    const t = translate({ repeat: "{{v}} and {{v}}" });
    expect(t("repeat" as never, { v: "x" })).toBe("x and x");
  });

  it("stringifies numeric params", () => {
    const t = translate({ total: "{{count}} books" });
    expect(t("total" as never, { count: 42 })).toBe("42 books");
  });

  it("leaves unknown placeholders in place", () => {
    const t = translate({ greet: "Hello {{name}}" });
    expect(t("greet" as never, { other: "x" })).toBe("Hello {{name}}");
  });
});

describe("getDictionarySync", () => {
  it("returns a populated dictionary per locale", () => {
    for (const locale of ["fr", "en"] as const) {
      const dict = getDictionarySync(locale);
      expect(Object.keys(dict).length).toBeGreaterThan(100);
    }
  });
});
