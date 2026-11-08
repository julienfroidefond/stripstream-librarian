import fs from "node:fs";
import path from "node:path";

import { uniqueName } from "../helpers";
import { ApiRequestError, apiRequest, seriesExists, deleteSeries } from "./api";
import { ensureBookLibrary } from "./metadata";

/**
 * Disposable book fixture for book-action specs.
 *
 * The CBZ is copied into the dedicated `e2e-books` library so the real scanner
 * creates the database row without pruning empty series from `e2e-metadata`
 * that other specs create in parallel. The API, indexer and tests must share a
 * filesystem, which is already required by `prepareMetadataLibrary`.
 */

const FIXTURE = path.resolve(__dirname, "../fixtures/library/E2E Series/E2E Series - T01.cbz");

export type BookSummary = {
  id: string;
  library_id: string;
  series_id: string | null;
  title: string;
  file_path: string | null;
};

export type DisposableBook = {
  id: string;
  title: string;
  libraryId: string;
  seriesId: string | null;
  directory: string;
  filePath: string;
};

type BooksPage = { items: BookSummary[]; total: number };

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Copy a small CBZ under a unique series folder, scan the book library and
 * wait for the indexer to expose the row through `GET /books`.
 */
export async function createDisposableBook(): Promise<DisposableBook> {
  const library = await ensureBookLibrary();
  const slug = uniqueName("000-e2e-book");
  const directory = path.join(library.root_path, slug);
  const filePath = path.join(directory, `${slug} - T01.cbz`);

  fs.mkdirSync(directory, { recursive: true });
  fs.copyFileSync(FIXTURE, filePath);

  try {
    await apiRequest(`/libraries/${library.id}/scan`, { method: "POST" });

    let found: BookSummary | undefined;
    const deadline = Date.now() + 45_000;
    while (Date.now() < deadline) {
      const page = await apiRequest<BooksPage>(
        `/books?library_id=${library.id}&q=${encodeURIComponent(slug)}&limit=20`
      );
      found = page.items.find((item) => item.library_id === library.id);
      if (found) break;
      await sleep(500);
    }

    if (!found) {
      throw new Error(
        `Le scan n'a pas importé ${filePath}. L'indexer voit-il ${library.root_path} ?`
      );
    }

    return {
      id: found.id,
      title: found.title,
      libraryId: library.id,
      seriesId: found.series_id,
      directory,
      filePath,
    };
  } catch (error) {
    fs.rmSync(directory, { recursive: true, force: true });
    throw error;
  }
}

export async function getBook(id: string): Promise<BookSummary> {
  return apiRequest<BookSummary>(`/books/${id}`);
}

export async function getBookOrNull(id: string): Promise<BookSummary | null> {
  try {
    return await getBook(id);
  } catch (error) {
    if (error instanceof ApiRequestError && error.status === 404) return null;
    throw error;
  }
}

/** Best-effort cleanup: deleting the book also prunes its now-empty series. */
export async function removeDisposableBook(book: DisposableBook): Promise<void> {
  await apiRequest(`/books/${book.id}`, { method: "DELETE" }).catch(() => undefined);
  if (book.seriesId && (await seriesExists(book.seriesId).catch(() => false))) {
    await deleteSeries(book.seriesId).catch(() => undefined);
  }
  fs.rmSync(book.directory, { recursive: true, force: true });
}
