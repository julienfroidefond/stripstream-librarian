import { describe, expect, it } from "vitest";

import { downloadStatusClass, downloadStatusLabel } from "@/lib/downloads/status";
import type { TranslationKey } from "@/lib/i18n/fr";

const t = (key: TranslationKey) => `t:${key}`;

describe("downloadStatusLabel", () => {
  it("maps every download status to its translation key", () => {
    expect(downloadStatusLabel("queued", t)).toBe("t:downloads.status.queued");
    expect(downloadStatusLabel("downloading", t)).toBe("t:downloads.status.downloading");
    expect(downloadStatusLabel("completed", t)).toBe("t:downloads.status.completed");
    expect(downloadStatusLabel("importing", t)).toBe("t:downloads.status.importing");
    expect(downloadStatusLabel("imported", t)).toBe("t:downloads.status.imported");
    expect(downloadStatusLabel("partial", t)).toBe("t:downloads.status.partial");
    expect(downloadStatusLabel("no_files_imported", t)).toBe("t:downloads.status.noFilesImported");
    expect(downloadStatusLabel("error", t)).toBe("t:downloads.status.error");
  });

  it("falls back to the raw status when it has no translation", () => {
    expect(downloadStatusLabel("dismissed", t)).toBe("t:dismissed");
  });
});

describe("downloadStatusClass", () => {
  it("uses a distinct badge class per status family", () => {
    expect(downloadStatusClass("queued")).toContain("bg-muted");
    expect(downloadStatusClass("downloading")).toContain("text-primary");
    expect(downloadStatusClass("completed")).toContain("text-warning");
    expect(downloadStatusClass("imported")).toContain("text-success");
    expect(downloadStatusClass("error")).toContain("text-destructive");
  });

  it("falls back to the muted badge class for unknown statuses", () => {
    expect(downloadStatusClass("dismissed")).toBe("bg-muted/30 text-muted-foreground");
  });
});
