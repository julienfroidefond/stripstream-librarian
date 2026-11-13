import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import path from "node:path";

/**
 * Contract helpers between `browserApi` and the Next proxy route handlers.
 *
 * Browser-side clients and route handlers are usually tested with the other
 * half mocked, so a client calling a URL no route handles goes unnoticed. These
 * helpers resolve a same-origin `/api/...` URL to its `app/api/**\/route.ts`
 * file the way the App Router does (static segments first, then a `[param]`
 * directory) and read which HTTP methods the file exports.
 */

const APP_DIR = path.resolve(import.meta.dirname, "../../../app");

function isDir(p: string): boolean {
  return existsSync(p) && statSync(p).isDirectory();
}

function resolveFrom(dir: string, segments: string[]): string | null {
  if (segments.length === 0) {
    const file = path.join(dir, "route.ts");
    return existsSync(file) ? file : null;
  }
  const [head, ...rest] = segments;
  const staticDir = path.join(dir, head);
  if (isDir(staticDir)) {
    const found = resolveFrom(staticDir, rest);
    if (found) return found;
  }
  for (const entry of readdirSync(dir)) {
    if (/^\[[^.\]][^\]]*\]$/.test(entry) && isDir(path.join(dir, entry))) {
      const found = resolveFrom(path.join(dir, entry), rest);
      if (found) return found;
    }
  }
  return null;
}

/** Route file handling `url` (path + optional query), or `null` when none does. */
export function resolveProxyRoute(url: string): string | null {
  const pathname = url.split("?")[0];
  const segments = pathname.split("/").filter(Boolean).map(decodeURIComponent);
  if (segments[0] !== "api") return null;
  return resolveFrom(APP_DIR, segments);
}

/** HTTP methods a route file exports (`export const POST` / `export async function POST`). */
export function exportedMethods(file: string): string[] {
  const source = readFileSync(file, "utf8");
  const methods = new Set<string>();
  const re = /export\s+(?:const|(?:async\s+)?function)\s+(GET|POST|PUT|PATCH|DELETE)\b/g;
  for (const match of source.matchAll(re)) methods.add(match[1]);
  return [...methods].sort();
}
