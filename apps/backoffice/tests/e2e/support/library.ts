import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { expect, type APIRequestContext } from "@playwright/test";

/**
 * Deterministic fixture library for local runs that opt in.
 *
 * A fresh database has no series and no books, which would skip most of the
 * catalog coverage. When `E2E_SEED_LIBRARY=1`, the setup copies the small CBZ
 * fixtures from `tests/e2e/fixtures/library` to a writable directory, registers
 * it through the API and waits for the indexer to analyse the books.
 *
 * Both the API and the indexer must see the same filesystem path: this assumes
 * they run natively on the same host. The Docker stack cannot see the host's
 * temporary directory, so by default local runs use the existing indexed data.
 */

const LIBRARY_NAME = "e2e-library";
const FIXTURE_DIR = path.resolve(__dirname, "../fixtures/library");

type Library = { id: string; name: string; root_path: string };

export function isLibrarySeedingEnabled(): boolean {
  const value = process.env.E2E_SEED_LIBRARY;
  return value === "1" || value === "true";
}

function targetRoot(): string {
  return process.env.E2E_LIBRARY_ROOT || path.join(os.tmpdir(), "stripstream-e2e-library");
}

/** Number of indexable files in the fixture library (so the poll stays in sync). */
function fixtureBookCount(): number {
  let count = 0;
  const walk = (dir: string): void => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (/\.(cbz|cbr|pdf|epub)$/i.test(entry.name)) count += 1;
    }
  };
  walk(FIXTURE_DIR);
  return count;
}

export async function seedLibrary(
  request: APIRequestContext,
  baseUrl: string,
  token: string
): Promise<void> {
  const root = targetRoot();
  fs.rmSync(root, { recursive: true, force: true });
  fs.mkdirSync(root, { recursive: true });
  fs.cpSync(FIXTURE_DIR, root, { recursive: true });
  // The API rejects symlinked paths it cannot canonicalise; compare real paths.
  const rootPath = fs.realpathSync(root);

  const headers = { Authorization: `Bearer ${token}` };

  const listRes = await request.get(`${baseUrl}/libraries`, { headers });
  expect(listRes.ok(), `liste des bibliothèques échouée (${listRes.status()})`).toBeTruthy();
  const libraries = (await listRes.json()) as Library[];

  // Recreate instead of reusing: the fixture contents may have changed between
  // commits, and a stale root path from another checkout cannot be updated.
  for (const library of libraries.filter(
    (item) => item.name === LIBRARY_NAME || item.root_path === rootPath
  )) {
    const deleteRes = await request.delete(`${baseUrl}/libraries/${library.id}`, { headers });
    expect(deleteRes.ok(), `suppression de la bibliothèque ${library.id} échouée`).toBeTruthy();
  }

  const createRes = await request.post(`${baseUrl}/libraries`, {
    headers,
    data: { name: LIBRARY_NAME, root_path: rootPath },
  });
  expect(createRes.ok(), `création de la bibliothèque échouée (${createRes.status()})`).toBeTruthy();
  const library = (await createRes.json()) as Library;

  const scanRes = await request.post(`${baseUrl}/libraries/${library.id}/scan`, { headers });
  expect(scanRes.ok(), `déclenchement du scan échoué (${scanRes.status()})`).toBeTruthy();

  const expectedBooks = Math.max(1, fixtureBookCount());
  try {
    await expect
      .poll(
        async () => {
          const booksRes = await request.get(
            `${baseUrl}/books?library_id=${library.id}&limit=1`,
            { headers }
          );
          if (!booksRes.ok()) return -1;
          const page = (await booksRes.json()) as { total?: number };
          return page.total ?? 0;
        },
        { timeout: 120_000, intervals: [500, 1_000, 2_000] }
      )
      .toBeGreaterThanOrEqual(expectedBooks);
  } catch {
    const jobsRes = await request.get(`${baseUrl}/jobs?limit=5`, { headers }).catch(() => null);
    const jobs = jobsRes?.ok() ? await jobsRes.text() : "jobs indisponibles";
    throw new Error(
      `Le scan de la bibliothèque E2E n'a pas produit les ${expectedBooks} livre(s) attendus. ` +
        `L'indexer tourne-t-il sur le même système de fichiers ? Derniers jobs : ${jobs}`
    );
  }
}
