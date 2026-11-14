/**
 * Couleurs des graphiques (recharts).
 *
 * Ce sont des variables CSS, pas des triplets HSL : le SVG se repeint donc au
 * changement de thème. Les valeurs codées en dur gardaient les teintes du thème
 * clair quand l'application passait en sombre, ce qui jurait avec la charte.
 *
 * Les tokens `--color-chart-*` sont définis dans `app/globals.css` (un jeu par
 * thème) et reprennent l'échelle du reader.
 */

/** Rôles fixes : lecture (non lu / en cours / lu), séries liées ou non. */
export const CHART_COLORS = {
  teal: "var(--color-chart-1)",
  amber: "var(--color-chart-2)",
  neutral: "var(--color-chart-3)",
  green: "var(--color-chart-4)",
  rose: "var(--color-chart-5)",
  blue: "var(--color-chart-6)",
  plum: "var(--color-chart-7)",
  copper: "var(--color-chart-8)",
} as const;

/** Palette ordonnée pour les séries catégorielles (formats, utilisateurs…). */
export const CHART_PALETTE: readonly string[] = [
  CHART_COLORS.teal,
  CHART_COLORS.green,
  CHART_COLORS.amber,
  CHART_COLORS.rose,
  CHART_COLORS.blue,
  CHART_COLORS.plum,
  CHART_COLORS.neutral,
  CHART_COLORS.copper,
];

/** Couleur de la n-ième série catégorielle, cyclique. */
export function chartColor(index: number): string {
  return CHART_PALETTE[index % CHART_PALETTE.length];
}
