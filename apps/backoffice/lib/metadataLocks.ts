/**
 * Keep only lockable keys explicitly set to `true`. The list of valid keys comes
 * from `GET /metadata/fields` through `useLockableFields`.
 *
 * When `allowed` is `null` the catalog is unavailable: keep every true entry
 * rather than dropping locks the user did not touch. The API sanitizes writes,
 * so forwarding a stale key is harmless, while silently clearing a valid lock
 * is not.
 */
export function pickLockableFields(
  locked: Record<string, boolean> | null | undefined,
  allowed: readonly string[] | null,
): Record<string, boolean> {
  const allowedKeys = allowed ? new Set(allowed) : null;
  return Object.fromEntries(
    Object.entries(locked ?? {}).filter(
      ([key, value]) => value === true && (allowedKeys === null || allowedKeys.has(key)),
    ),
  );
}
