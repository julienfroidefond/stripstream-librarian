// @vitest-environment node
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative, sep } from "node:path";

import { describe, expect, it } from "vitest";

const ROOT = join(__dirname, "../../../..");
const SCANNED = ["app", "lib"];

/**
 * Files still allowed to call Next's invalidation primitives directly.
 * Keep this list empty; each entry is debt with a reason.
 */
const ALLOWED = new Set<string>([]);

function sources(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return sources(path);
    return /\.(ts|tsx)$/.test(name) ? [path] : [];
  });
}

function scanned(): Array<{ file: string; text: string }> {
  return SCANNED.flatMap((dir) => sources(join(ROOT, dir)))
    .map((path) => ({ file: relative(ROOT, path).split(sep).join("/"), text: readFileSync(path, "utf8") }))
    .filter(({ file }) => !file.startsWith("lib/cache/"));
}

describe("cache invalidation seam", () => {
  it("keeps revalidateTag / revalidatePath / updateTag inside lib/cache", () => {
    const offenders = scanned()
      .filter(({ file }) => !ALLOWED.has(file))
      .filter(({ text }) => /\b(revalidateTag|revalidatePath|updateTag)\s*\(/.test(text))
      .map(({ file }) => file);
    expect(offenders).toEqual([]);
  });

  it("builds every fetch cache tag with cacheTags", () => {
    const offenders = scanned()
      .filter(({ text }) => /tags:\s*\[[^\]]*["'`]/.test(text))
      .map(({ file }) => file);
    expect(offenders).toEqual([]);
  });

  it("keeps the allowlist honest", () => {
    const files = new Map(scanned().map(({ file, text }) => [file, text]));
    for (const file of ALLOWED) {
      expect(files.get(file), file).toMatch(/\b(revalidateTag|revalidatePath|updateTag)\s*\(/);
    }
  });
});
