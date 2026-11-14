/**
 * Translations of dynamic keys (`seriesStatus.<status>`, `field.<name>`).
 *
 * `t()` returns the key itself when it is missing, which is truthy: a
 * `t(key) || raw` fallback never triggers and the UI shows `seriesStatus.foo`.
 * These helpers detect the miss and fall back to the raw value instead.
 * Pure: usable from Server and Client Components.
 */

import type { TranslationKey } from "./fr";

type Translate = (key: TranslationKey, params?: Record<string, string | number>) => string;

/** `t(key)` when the dictionary knows `key`, otherwise `fallback`. */
export function translateOr(t: Translate, key: string, fallback: string): string {
  const translated = t(key as TranslationKey);
  return translated && translated !== key ? translated : fallback;
}

/** Localized series status (`ongoing` → « En cours »), or the raw status when unknown. */
export function translateSeriesStatus(t: Translate, status: string): string {
  return translateOr(t, `seriesStatus.${status}`, status);
}

/** Localized metadata field name, or the raw field name when unknown. */
export function translateField(t: Translate, field: string): string {
  return translateOr(t, `field.${field}`, field);
}
