import { removeMetadataLibrary } from "./support/metadata";

/**
 * Nettoyage global : les bibliothèques temporaires des fixtures ne doivent pas
 * survivre au run. Best-effort pour ne pas masquer le résultat des tests ; le
 * prochain setup les supprimera de toute façon.
 */
export default async function globalTeardown(): Promise<void> {
  try {
    await removeMetadataLibrary();
  } catch (error) {
    console.warn("[e2e] nettoyage des bibliothèques temporaires échoué :", error);
  }
}
