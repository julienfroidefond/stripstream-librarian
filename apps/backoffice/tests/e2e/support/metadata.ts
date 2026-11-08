import fs from "node:fs";
import path from "node:path";

import { type Page } from "@playwright/test";

import { openSeriesDetail, uniqueName } from "../helpers";
import {
  ApiRequestError,
  createLibrary,
  createSeries,
  deleteLibrary,
  deleteSeries,
  listArchivedSeries,
  listLibraries,
  updateSeries,
  type LibrarySummary,
} from "./api";

/**
 * Bibliothèques temporaires réservées aux fixtures.
 *
 * `e2e-metadata` ne contient que des séries vides (auteurs, genres). Les
 * parcours qui déclenchent un scan ou un rebuild utilisent des racines
 * séparées (`e2e-books`, `e2e-jobs`) : l'indexer supprime les séries orphelines
 * après chaque scan, ce qui effacerait les fixtures créées en parallèle par les
 * autres specs si elles partageaient la même bibliothèque.
 *
 * Les trois bibliothèques sont recréées par `auth.setup.ts`, puis supprimées
 * par le teardown global. Les séries de test sont archivées à la suppression
 * (comportement de l'API) ; les archives restantes sont purgées en recréant les
 * séries avant de supprimer chaque bibliothèque.
 */

export const METADATA_LIBRARY_NAME = "e2e-metadata";
export const BOOK_LIBRARY_NAME = "e2e-books";
export const JOBS_LIBRARY_NAME = "e2e-jobs";

const TEMP_DIR_NAME = "stripstream-e2e-metadata";

const AUXILIARY_LIBRARIES = [
  { name: BOOK_LIBRARY_NAME, dirName: "stripstream-e2e-books" },
  { name: JOBS_LIBRARY_NAME, dirName: "stripstream-e2e-jobs" },
] as const;

const MANAGED_LIBRARY_NAMES = new Set<string>([
  METADATA_LIBRARY_NAME,
  ...AUXILIARY_LIBRARIES.map((library) => library.name),
]);

const MANAGED_ROOT_NAMES = new Set<string>([
  TEMP_DIR_NAME,
  ...AUXILIARY_LIBRARIES.map((library) => library.dirName),
]);

export type MetadataSeries = {
  libraryId: string;
  seriesId: string;
  name: string;
};

/**
 * Répertoires candidats, du plus isolé au plus général. Le repli sur `/tmp`
 * permet aux stacks Docker (qui ne voient pas le répertoire créé sur l'hôte)
 * de trouver un chemin existant. `E2E_METADATA_LIBRARY_ROOT` force un chemin.
 */
function candidateRoots(): string[] {
  const override = process.env.E2E_METADATA_LIBRARY_ROOT?.trim();
  if (override) return [override];

  return [
    path.join("/tmp", TEMP_DIR_NAME),
    path.join("/var/tmp", TEMP_DIR_NAME),
    path.join("/dev/shm", TEMP_DIR_NAME),
    "/tmp",
    "/var/tmp",
    "/dev/shm",
  ];
}

async function findLibraryByName(name: string): Promise<LibrarySummary | null> {
  const libraries = await listLibraries();
  return libraries.find((library) => library.name === name) ?? null;
}

/**
 * Purge les archives de la bibliothèque en recréant les séries portant le même
 * nom : l'API restaure les métadonnées puis supprime la ligne `archived_series`.
 * Les séries recréées seront supprimées en cascade avec la bibliothèque.
 */
async function purgeArchivedSeries(libraryId: string): Promise<void> {
  const archived = await listArchivedSeries();
  const rows = archived.filter((item) => item.library_id === libraryId);
  for (const row of rows) {
    await createSeries(libraryId, row.name);
  }
}

function removeManagedRoot(rootPath: string): void {
  if (!MANAGED_ROOT_NAMES.has(path.basename(rootPath))) return;
  try {
    fs.rmSync(rootPath, { recursive: true, force: true });
  } catch {
    // Nettoyage best-effort : un répertoire résiduel ne casse pas la suite.
  }
}

async function removeLibrary(library: LibrarySummary): Promise<void> {
  // Toujours purger avant de supprimer : `DELETE /libraries` ne nettoie pas
  // `archived_series` (pas de clé étrangère).
  await purgeArchivedSeries(library.id);
  await deleteLibrary(library.id);
  removeManagedRoot(library.root_path);
}

/** Supprime les bibliothèques temporaires et purge leurs archives. */
export async function removeMetadataLibrary(): Promise<void> {
  const libraries = await listLibraries();
  for (const library of libraries) {
    if (!MANAGED_LIBRARY_NAMES.has(library.name)) continue;
    await removeLibrary(library);
  }
}

/**
 * Recrée la bibliothèque temporaire. À appeler depuis le projet `setup` :
 * les workers parallèles se contentent ensuite de la retrouver.
 */
export async function prepareMetadataLibrary(): Promise<LibrarySummary> {
  await removeMetadataLibrary();

  const failures: string[] = [];
  let metadataLibrary: LibrarySummary | null = null;
  for (const candidate of candidateRoots()) {
    try {
      fs.mkdirSync(candidate, { recursive: true });
    } catch (error) {
      failures.push(`${candidate} : répertoire inaccessible (${String(error)})`);
      continue;
    }

    try {
      metadataLibrary = await createLibrary(METADATA_LIBRARY_NAME, candidate);
      break;
    } catch (error) {
      if (error instanceof ApiRequestError && [400, 409].includes(error.status)) {
        failures.push(`${candidate} : ${error.message}`);
        continue;
      }
      throw error;
    }
  }

  if (!metadataLibrary) {
    throw new Error(
      `Impossible de créer la bibliothèque ${METADATA_LIBRARY_NAME}. ` +
        `L'API et les tests doivent voir le même système de fichiers, ou définis ` +
        `E2E_METADATA_LIBRARY_ROOT vers un répertoire existant côté API.\n` +
        failures.join("\n")
    );
  }

  // Par défaut, les racines auxiliaires sont des sœurs de `e2e-metadata`.
  // Pour un chemin forcé (ou le repli sur `/tmp` lui-même), on les imbrique
  // sous la racine de métadonnées afin de rester dans un répertoire visible.
  const parent = path.dirname(metadataLibrary.root_path);
  const useParent =
    parent !== path.parse(parent).root &&
    path.basename(metadataLibrary.root_path) === TEMP_DIR_NAME;
  const auxiliaryBase = useParent ? parent : metadataLibrary.root_path;

  for (const auxiliary of AUXILIARY_LIBRARIES) {
    const rootPath = path.join(auxiliaryBase, auxiliary.dirName);
    fs.mkdirSync(rootPath, { recursive: true });
    await createLibrary(auxiliary.name, rootPath);
  }

  return metadataLibrary;
}

async function ensureLibrary(name: string): Promise<LibrarySummary> {
  const library = await findLibraryByName(name);
  if (!library) {
    throw new Error(
      `Bibliothèque ${name} introuvable. Le projet « setup » ` +
        `(auth.setup.ts) doit la créer avant les specs.`
    );
  }
  return library;
}

/** Retrouve la bibliothèque créée par le setup (garde-fou explicite). */
export async function ensureMetadataLibrary(): Promise<LibrarySummary> {
  return ensureLibrary(METADATA_LIBRARY_NAME);
}

/** Bibliothèque dédiée aux livres jetables (scan réel, pas de fixtures partagées). */
export async function ensureBookLibrary(): Promise<LibrarySummary> {
  return ensureLibrary(BOOK_LIBRARY_NAME);
}

/** Bibliothèque vide dédiée aux rebuilds : aucun orphelin d'autres specs. */
export async function ensureJobsLibrary(): Promise<LibrarySummary> {
  return ensureLibrary(JOBS_LIBRARY_NAME);
}

/**
 * Crée une série déterministe dans la bibliothèque temporaire, puis lui
 * applique des auteurs et des genres uniques au run.
 */
export async function createMetadataSeries(
  options: { authors?: string[]; genres?: string[]; name?: string } = {}
): Promise<MetadataSeries> {
  const library = await ensureMetadataLibrary();
  const name = options.name ?? uniqueName("E2E metadata series");
  const created = await createSeries(library.id, name);

  try {
    await updateSeries(created.series_id, {
      new_name: name,
      authors: options.authors ?? [],
      genres: options.genres ?? [],
    });
  } catch (error) {
    await deleteSeries(created.series_id).catch(() => undefined);
    throw error;
  }

  return { libraryId: library.id, seriesId: created.series_id, name };
}

/**
 * Supprime la série de test sans laisser d'auteurs ou de genres dans l'archive.
 * L'archive vide est purgée avec la bibliothèque au teardown.
 */
export async function removeMetadataSeries(series: MetadataSeries): Promise<void> {
  await updateSeries(series.seriesId, {
    new_name: series.name,
    authors: [],
    genres: [],
  }).catch(() => undefined);
  await deleteSeries(series.seriesId).catch(() => undefined);
}

/** Open a fixture series detail page from the metadata library. */
export async function openMetadataSeries(page: Page, series: MetadataSeries): Promise<void> {
  await openSeriesDetail(page, {
    name: series.name,
    seriesId: series.seriesId,
    libraryName: METADATA_LIBRARY_NAME,
  });
}
