import fs from "node:fs";
import path from "node:path";

import { uniqueName } from "../helpers";
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
 * Bibliothèque réservée aux fixtures de métadonnées (auteurs, genres).
 *
 * Les tests ne doivent pas modifier les données réelles : cette bibliothèque
 * temporaire ne contient que des séries créées par la suite. Elle est recréée
 * par `auth.setup.ts`, puis supprimée par le teardown global. Les séries de
 * test sont archivées à la suppression (comportement de l'API) ; leurs
 * métadonnées sont donc vidées avant, et les archives restantes sont purgées
 * en recréant les séries avant de supprimer la bibliothèque.
 */

export const METADATA_LIBRARY_NAME = "e2e-metadata";
const TEMP_DIR_NAME = "stripstream-e2e-metadata";

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

async function findMetadataLibrary(): Promise<LibrarySummary | null> {
  const libraries = await listLibraries();
  return libraries.find((library) => library.name === METADATA_LIBRARY_NAME) ?? null;
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
  if (path.basename(rootPath) !== TEMP_DIR_NAME) return;
  try {
    fs.rmSync(rootPath, { recursive: true, force: true });
  } catch {
    // Nettoyage best-effort : un répertoire résiduel ne casse pas la suite.
  }
}

/** Supprime la bibliothèque temporaire et purge ses archives. */
export async function removeMetadataLibrary(): Promise<void> {
  const library = await findMetadataLibrary();
  if (!library) return;

  // Toujours purger avant de supprimer : `DELETE /libraries` ne nettoie pas
  // `archived_series` (pas de clé étrangère).
  await purgeArchivedSeries(library.id);
  await deleteLibrary(library.id);
  removeManagedRoot(library.root_path);
}

/**
 * Recrée la bibliothèque temporaire. À appeler depuis le projet `setup` :
 * les workers parallèles se contentent ensuite de la retrouver.
 */
export async function prepareMetadataLibrary(): Promise<LibrarySummary> {
  await removeMetadataLibrary();

  const failures: string[] = [];
  for (const candidate of candidateRoots()) {
    try {
      fs.mkdirSync(candidate, { recursive: true });
    } catch (error) {
      failures.push(`${candidate} : répertoire inaccessible (${String(error)})`);
      continue;
    }

    try {
      return await createLibrary(METADATA_LIBRARY_NAME, candidate);
    } catch (error) {
      if (error instanceof ApiRequestError && [400, 409].includes(error.status)) {
        failures.push(`${candidate} : ${error.message}`);
        continue;
      }
      throw error;
    }
  }

  throw new Error(
    `Impossible de créer la bibliothèque ${METADATA_LIBRARY_NAME}. ` +
      `L'API et les tests doivent voir le même système de fichiers, ou définis ` +
      `E2E_METADATA_LIBRARY_ROOT vers un répertoire existant côté API.\n` +
      failures.join("\n")
  );
}

/** Retrouve la bibliothèque créée par le setup (garde-fou explicite). */
export async function ensureMetadataLibrary(): Promise<LibrarySummary> {
  const library = await findMetadataLibrary();
  if (!library) {
    throw new Error(
      `Bibliothèque ${METADATA_LIBRARY_NAME} introuvable. Le projet « setup » ` +
        `(auth.setup.ts) doit la créer avant les specs.`
    );
  }
  return library;
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
