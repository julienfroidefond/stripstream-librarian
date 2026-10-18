import type { Transition } from "framer-motion";

/**
 * Langage de mouvement « projection », partagé avec le lecteur StripStream.
 *
 * Une seule famille de courbes pour toute l'application : un départ vif et une
 * arrivée posée, comme un projecteur qui se cale. Les valeurs vivent ici pour
 * que chaque surface animée côté JS raconte la même chose que le CSS
 * (`--ease-projection`, `--motion-*` dans `globals.css`).
 *
 * Ce module n'importe rien de framer-motion à l'exécution : il reste
 * importable depuis un Server Component sans tirer la librairie dedans.
 */

/** Courbe signature : départ vif, arrivée posée. */
export const EASE_PROJECTION: [number, number, number, number] = [0.22, 1, 0.36, 1];

/** Courbe d'appui pour les micro-interactions (survol, focus). */
export const EASE_OUT: [number, number, number, number] = [0.16, 1, 0.3, 1];

/** Perspective commune aux gestes en 3D (surfaces qui pivotent). */
export const PERSPECTIVE = 1100;

/** Durées, alignées sur les tokens CSS. */
export const DURATION = {
  fast: 0.24,
  base: 0.56,
  slow: 0.78,
  cinema: 1.05,
} as const;

/** Transition d'entrée standard : longue, pour qu'un geste se lise. */
export const projection: Transition = { duration: DURATION.slow, ease: EASE_PROJECTION };

/** Ressort posé, sans rebond : pour les surfaces qui se calent (panneaux). */
export const springSettle: Transition = { type: "spring", stiffness: 320, damping: 32, mass: 0.9 };

/** Ressort nerveux, léger dépassement : pour les gestes courts (tiroirs). */
export const springSnap: Transition = { type: "spring", stiffness: 420, damping: 26, mass: 0.7 };
