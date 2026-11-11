/**
 * Une seule réponse à « cette source d'acquisition est-elle utilisable ? ».
 *
 * Le module est pur : il reçoit un instantané des réglages, du statut Telegram
 * et des fournisseurs DDL, puis applique les règles. Les lectures réseau vivent
 * dans `acquisitionReadiness.server.ts`, les composants ne manipulent que la
 * valeur résolue.
 *
 * Le flag maître `downloads` gouverne toutes les capacités. Chaque source
 * applique ensuite sa propre règle en AND, ce qui évite qu'une intégration
 * laissée active continue d'apparaître quand les téléchargements sont coupés.
 */

import { isClientConfigured } from "./downloadClient";

export interface AcquisitionReadiness {
  /** Flag maître : sans lui, aucune autre capacité n'est disponible. */
  downloads: boolean;
  prowlarr: boolean;
  qbittorrent: boolean;
  alldebrid: boolean;
  telegram: boolean;
  ddl: boolean;
}

export interface AcquisitionReadinessSnapshot {
  /** Carte complète renvoyée par `GET /settings`, ou `null` si elle est indisponible. */
  settings: Record<string, unknown> | null;
  /** Statut du moniteur Telegram (configuration + session autorisée). */
  telegram?: { configured?: boolean; authorized?: boolean } | null;
  /** Liste des fournisseurs DDL avec leur état d'activation. */
  ddlProviders?: ReadonlyArray<{ enabled?: boolean }> | null;
}

export const EMPTY_ACQUISITION_READINESS: AcquisitionReadiness = Object.freeze({
  downloads: false,
  prowlarr: false,
  qbittorrent: false,
  alldebrid: false,
  telegram: false,
  ddl: false,
});

function asRecord(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

/**
 * Les flags de l'API sont stockés sous `{ enabled: boolean }`; accepter aussi un
 * booléen brut garde le resolver tolérant aux anciennes valeurs et aux tests.
 */
function isEnabled(value: unknown): boolean {
  if (typeof value === "boolean") return value;
  return asRecord(value)?.enabled === true;
}

/**
 * Prowlarr est utilisable quand il n'est pas explicitement désactivé et que
 * l'URL comme la clé d'API sont renseignées. Une configuration partielle ne
 * doit pas faire apparaître les recherches dans les menus.
 */
export function isProwlarrConfigured(data: Record<string, unknown> | null): boolean {
  if (!data || data.enabled === false) return false;
  return (
    typeof data.url === "string" &&
    data.url.trim().length > 0 &&
    typeof data.api_key === "string" &&
    data.api_key.trim().length > 0
  );
}

export function resolveAcquisitionReadiness(
  snapshot: AcquisitionReadinessSnapshot
): AcquisitionReadiness {
  const settings = snapshot.settings;
  const downloads = isEnabled(settings?.downloads_enabled);

  if (!downloads) {
    // Le flag maître coupe tout, y compris Telegram et DDL qui étaient
    // auparavant évalués indépendamment de lui.
    return { ...EMPTY_ACQUISITION_READINESS };
  }

  const telegram = snapshot.telegram;
  return {
    downloads: true,
    prowlarr: isProwlarrConfigured(asRecord(settings?.prowlarr)),
    qbittorrent: isClientConfigured("qbittorrent", asRecord(settings?.qbittorrent)),
    alldebrid: isClientConfigured("alldebrid", asRecord(settings?.alldebrid)),
    telegram: telegram?.configured === true && telegram?.authorized === true,
    ddl: (snapshot.ddlProviders ?? []).some((provider) => provider.enabled === true),
  };
}
