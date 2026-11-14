import { cache } from "react";

import { fetchDdlProviders, fetchSettingsSnapshot, getTelegramMonitorStatus } from "../api";
import {
  EMPTY_ACQUISITION_READINESS,
  resolveAcquisitionReadiness,
  type AcquisitionReadiness,
} from "./readiness";

/**
 * Charge l'instantané nécessaire à la readiness puis délègue toute la décision
 * au resolver pur. Une lecture des settings couvre downloads/Prowlarr/clients ;
 * les deux autres sources restent indépendantes afin qu'une panne Telegram ou
 * DDL ne désactive pas Prowlarr.
 */
async function fetchAcquisitionReadiness(): Promise<AcquisitionReadiness> {
  const [settings, telegram, ddlProviders] = await Promise.all([
    fetchSettingsSnapshot().catch(() => null),
    getTelegramMonitorStatus({ revalidate: 60 }).catch(() => null),
    fetchDdlProviders({ revalidate: 60 }).catch(() => []),
  ]);

  // Sans la carte de réglages, on ne peut rien affirmer : toutes les sources
  // restent masquées plutôt que d'exposer une action qui échouera.
  if (!settings) return { ...EMPTY_ACQUISITION_READINESS };

  return resolveAcquisitionReadiness({ settings, telegram, ddlProviders });
}

/**
 * Résolution mémoïsée pour la durée d'un rendu serveur : le layout et la page
 * partagent ainsi une seule lecture, même en impersonation où le cache Next est
 * contourné.
 */
export const loadAcquisitionReadiness = cache(fetchAcquisitionReadiness);

/** Version non mémoïsée réservée aux tests du chargeur. @internal */
export const loadAcquisitionReadinessUncached = fetchAcquisitionReadiness;
