import fs from "node:fs";
import path from "node:path";

import { uniqueName } from "../helpers";
import {
  ApiRequestError,
  apiRequest,
  deleteSeries,
  getSeriesDetails,
  seriesExists,
} from "./api";
import { ensureBookLibrary } from "./metadata";

/**
 * Disposable book fixtures for book-action and reading specs.
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
  file_path?: string | null;
  locked_fields?: Record<string, boolean> | null;
};

export type DisposableBook = {
  id: string;
  title: string;
  libraryId: string;
  seriesId: string | null;
  directory: string;
  filePath: string;
};

export type DisposableSeries = {
  seriesId: string;
  name: string;
  libraryId: string;
  directory: string;
  books: BookSummary[];
};

type BooksPage = { items: BookSummary[]; total: number };

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function scanAndWait(
  libraryId: string,
  directory: string,
  slug: string,
  volumeCount: number
): Promise<BookSummary[]> {
  await apiRequest(`/libraries/${libraryId}/scan`, { method: "POST" });

  let found: BookSummary[] = [];
  const deadline = Date.now() + 45_000;
  while (Date.now() < deadline) {
    const page = await apiRequest<BooksPage>(
      `/books?library_id=${libraryId}&q=${encodeURIComponent(slug)}&limit=50`
    );
    found = page.items.filter((item) => item.library_id === libraryId);
    if (found.length >= volumeCount) return found;
    await sleep(500);
  }

  throw new Error(
    `Le scan n'a pas importé ${volumeCount} livre(s) sous ${directory}. ` +
      `L'indexer voit-il ${directory} ?`
  );
}

async function deleteBooks(books: BookSummary[], seriesId: string): Promise<void> {
  for (const book of books) {
    await apiRequest(`/books/${book.id}`, { method: "DELETE" }).catch(() => undefined);
  }
  if (await seriesExists(seriesId).catch(() => false)) {
    await deleteSeries(seriesId).catch(() => undefined);
  }
}

/**
 * Copy a small CBZ under a unique series folder, scan the book library and
 * wait for the indexer to expose its row through `GET /books`.
 */
export async function createDisposableBook(): Promise<DisposableBook> {
  const series = await createDisposableSeries(1);
  const book = series.books[0];
  const slug = path.basename(series.directory);

  return {
    id: book.id,
    title: book.title,
    libraryId: series.libraryId,
    seriesId: series.seriesId,
    directory: series.directory,
    filePath: path.join(series.directory, `${slug} - T01.cbz`),
  };
}

/**
 * Create a disposable series with several scanned volumes. Used by specs that
 * need to exercise per-volume reading state and whole-series actions.
 */
export async function createDisposableSeries(volumeCount = 2): Promise<DisposableSeries> {
  if (volumeCount < 1) throw new Error("volumeCount doit être supérieur à zéro");

  const library = await ensureBookLibrary();
  const slug = uniqueName("000-e2e-series");
  const directory = path.join(library.root_path, slug);

  fs.mkdirSync(directory, { recursive: true });
  for (let volume = 1; volume <= volumeCount; volume += 1) {
    const fileName = `${slug} - T${String(volume).padStart(2, "0")}.cbz`;
    fs.copyFileSync(FIXTURE, path.join(directory, fileName));
  }

  let books: BookSummary[] = [];
  let seriesId: string | null = null;
  try {
    books = await scanAndWait(library.id, directory, slug, volumeCount);
    seriesId = books[0]?.series_id ?? null;
    if (!seriesId || books.some((book) => book.series_id !== seriesId)) {
      throw new Error(`Le scan a importé des livres sans série commune sous ${directory}`);
    }

    const details = await getSeriesDetails(seriesId);
    return {
      seriesId,
      name: details.name,
      libraryId: library.id,
      directory,
      books,
    };
  } catch (error) {
    if (seriesId) {
      await deleteBooks(books, seriesId);
    }
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

/** Best-effort cleanup for a multi-volume disposable series. */
export async function removeDisposableSeries(series: DisposableSeries): Promise<void> {
  await deleteBooks(series.books, series.seriesId);
  fs.rmSync(series.directory, { recursive: true, force: true });
}
