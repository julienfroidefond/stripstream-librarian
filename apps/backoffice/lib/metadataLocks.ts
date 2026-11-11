/**
 * Field keys the API can lock against external metadata sync. Keep in sync with
 * `apps/api/src/metadata/field_policy.rs`; the admin API exposes the same list
 * on `GET /metadata/fields`.
 */
export const SERIES_LOCKABLE_FIELDS = [
  "description",
  "authors",
  "publishers",
  "start_year",
  "total_volumes",
  "status",
  "cover_url",
] as const;

export const BOOK_LOCKABLE_FIELDS = [
  "authors",
  "language",
  "isbn",
  "publish_date",
  "summary",
] as const;

/**
 * Keep only known lockable keys explicitly set to `true`. Drops stale keys left
 * by older UI versions (title, series, volume, genres…) and `false` entries,
 * which the API treats as absent.
 */
export function pickLockableFields(
  locked: Record<string, boolean> | null | undefined,
  allowed: readonly string[],
): Record<string, boolean> {
  const allowedKeys = new Set(allowed);
  return Object.fromEntries(
    Object.entries(locked ?? {}).filter(
      ([key, value]) => value === true && allowedKeys.has(key),
    ),
  );
}
