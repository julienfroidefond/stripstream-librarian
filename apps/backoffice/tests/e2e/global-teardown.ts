import { METADATA_LIBRARY_NAME, removeMetadataLibrary } from "./support/metadata";

/**
 * Nettoyage global : la bibliothèque temporaire des fixtures de métadonnées
 * ne doit pas survivre au run. Best-effort pour ne pas masquer le résultat des
 * tests ; le prochain setup la supprimera de toute façon.
 */
export default async function globalTeardown(): Promise<void> {
  try {
    await removeMetadataLibrary();
  } catch (error) {
    console.warn(`[e2e] nettoyage de ${METADATA_LIBRARY_NAME} échoué :`, error);
  }
}
