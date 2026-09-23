/**
 * Client-side search helpers.
 *
 * These mirror the database normalization used by the Rust API (`norm_text`,
 * migration 0115): lowercase + strip diacritics, so "asterix" matches "Astérix".
 * They are only a best-effort UI complement — the series search itself is
 * delegated to the API, which applies the authoritative `unaccent` normalization.
 */

/** Lowercase + remove diacritics, e.g. "Éditions Astérix" → "editions asterix". */
export function normalizeSearchText(value: string): string {
  return value
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase()
    .trim();
}

/** True when `value` contains `query`, ignoring case and accents. Empty query matches all. */
export function matchesSearchText(value: string, query: string): boolean {
  const normalizedQuery = normalizeSearchText(query);
  if (!normalizedQuery) return true;
  return normalizeSearchText(value).includes(normalizedQuery);
}
