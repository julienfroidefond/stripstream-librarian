/**
 * Préférence d'affichage (grille / liste) des listes de livres et de séries.
 *
 * Stockée dans un cookie (comme les filtres de `LiveSearchForm`) plutôt que
 * dans l'URL : les filtres réécrivent les searchParams à chaque changement et
 * écraseraient un éventuel paramètre `layout`. Le cookie est donc à la fois
 * lu côté serveur (rendu) et écrit côté client (bouton).
 */

export const VIEW_MODE_COOKIE = "view_mode";

export type ViewMode = "grid" | "list";

/** Cookie value → mode, en tolérant toute valeur inconnue (défaut : grille). */
export function parseViewMode(value: string | undefined | null): ViewMode {
  return value === "list" ? "list" : "grid";
}

/** Mode → cookie value. */
export function serializeViewMode(mode: ViewMode): string {
  return mode;
}

/** Durée de vie du cookie (1 an), en secondes. */
export const VIEW_MODE_MAX_AGE = 60 * 60 * 24 * 365;
