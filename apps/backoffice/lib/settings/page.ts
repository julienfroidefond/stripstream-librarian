/**
 * Chargement de la page `/settings`.
 *
 * Server-only : aucun React, aucun JSX. Le module possède tout le fan-out de
 * fetch de la page, qui ne fait plus que de la composition, et rend un modèle
 * unique à `SettingsPage`.
 *
 * Politique de dégradation — une seule, jamais un `.catch` local :
 * - aucune section n'est fatale : la page s'affiche toujours ;
 * - une section en échec vaut `null` (statistiques, versions, réglages) ou une
 *   liste vide (utilisateurs, mappings, statuts, bibliothèques), et son nom
 *   est ajouté à `model.degraded`. Aucune valeur n'est inventée : pas de
 *   répertoire de cache ou de miniatures par défaut présenté comme réel ;
 * - `settings` dégradé ⇒ `model.settings === null` : le brouillon est amorcé
 *   avec les défauts mais passe en lecture seule. Sans cela, ces défauts
 *   seraient pris pour des valeurs acquittées et la première sauvegarde
 *   enverrait une section complète qui écraserait la configuration stockée.
 */

import {
  fetchApiVersion,
  fetchIndexerVersion,
  fetchLibraries,
  fetchProviderStatuses,
  fetchSeriesStatuses,
  fetchStatusMappings,
  fetchUsers,
  getCacheStats,
  getSettings,
  getThumbnailStats,
} from "@/lib/api";
import type {
  CacheStats,
  LibraryDto,
  StatusMappingDto,
  ThumbnailStats,
  UserDto,
} from "@/lib/api";
import packageJson from "@/package.json";

/** Section whose failure degrades the page instead of failing it. */
export type SettingsSectionName =
  | "settings"
  | "cacheStats"
  | "thumbnailStats"
  | "users"
  | "statusMappings"
  | "seriesStatuses"
  | "providerStatuses"
  | "libraries"
  | "apiVersion"
  | "indexerVersion";

export type SettingsVersions = {
  /** `null` when the service did not answer: the UI shows it as unknown. */
  api: string | null;
  indexer: string | null;
  backoffice: string;
};

/** Everything the settings page renders, already resolved. */
export type SettingsPageModel = {
  /**
   * Stored `app_settings` map, or `null` when it could not be read. `null`
   * switches the draft to read-only: nothing may be saved over a config the
   * page has not seen.
   */
  settings: Record<string, unknown> | null;
  cacheStats: CacheStats | null;
  thumbnailStats: ThumbnailStats | null;
  users: UserDto[];
  statusMappings: StatusMappingDto[];
  seriesStatuses: string[];
  providerStatuses: string[];
  libraries: LibraryDto[];
  versions: SettingsVersions;
  /** Sections that failed, in declaration order. */
  degraded: SettingsSectionName[];
};

type Outcome<T> = { ok: true; value: T } | { ok: false };

/** Resolves to a tagged outcome instead of rejecting. */
function optional<T>(promise: Promise<T>): Promise<Outcome<T>> {
  return promise.then(
    (value) => ({ ok: true as const, value }),
    () => ({ ok: false as const }),
  );
}

export async function loadSettingsPage(): Promise<SettingsPageModel> {
  const degraded: SettingsSectionName[] = [];

  const [
    settings,
    cacheStats,
    thumbnailStats,
    users,
    statusMappings,
    seriesStatuses,
    providerStatuses,
    libraries,
    apiVersion,
    indexerVersion,
  ] = await Promise.all([
    optional(getSettings()),
    optional(getCacheStats()),
    optional(getThumbnailStats()),
    optional(fetchUsers()),
    optional(fetchStatusMappings()),
    optional(fetchSeriesStatuses()),
    optional(fetchProviderStatuses()),
    optional(fetchLibraries()),
    optional(fetchApiVersion()),
    optional(fetchIndexerVersion()),
  ]);

  function orNull<T>(name: SettingsSectionName, outcome: Outcome<T>): T | null {
    if (outcome.ok) return outcome.value;
    degraded.push(name);
    return null;
  }

  function orEmpty<T>(name: SettingsSectionName, outcome: Outcome<T[]>): T[] {
    return orNull(name, outcome) ?? [];
  }

  return {
    settings: orNull("settings", settings),
    cacheStats: orNull("cacheStats", cacheStats),
    thumbnailStats: orNull("thumbnailStats", thumbnailStats),
    users: orEmpty("users", users),
    statusMappings: orEmpty("statusMappings", statusMappings),
    seriesStatuses: orEmpty("seriesStatuses", seriesStatuses),
    providerStatuses: orEmpty("providerStatuses", providerStatuses),
    libraries: orEmpty("libraries", libraries),
    versions: {
      api: orNull("apiVersion", apiVersion),
      indexer: orNull("indexerVersion", indexerVersion),
      backoffice: packageJson.version,
    },
    degraded,
  };
}
