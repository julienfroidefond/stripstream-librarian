/**
 * Session secret reader shared by the proxy (edge) and `session.ts` (node).
 * Pure: no `next/headers`, so the proxy can import it.
 */

export const SESSION_SECRET_MIN_LENGTH = 32;

/**
 * Returns the encoded `SESSION_SECRET`, or `null` when it is missing or
 * shorter than {@link SESSION_SECRET_MIN_LENGTH} characters. Callers must fail
 * closed on `null`: there is no insecure fallback.
 */
export function readSessionSecret(
  value: string | undefined = process.env.SESSION_SECRET,
): Uint8Array | null {
  if (!value || value.length < SESSION_SECRET_MIN_LENGTH) return null;
  return new TextEncoder().encode(value);
}

/** Same as {@link readSessionSecret} but throws when the secret is unusable. */
export function requireSessionSecret(
  value: string | undefined = process.env.SESSION_SECRET,
): Uint8Array {
  const secret = readSessionSecret(value);
  if (!secret) {
    throw new Error(
      `SESSION_SECRET env var is required (min ${SESSION_SECRET_MIN_LENGTH} characters)`,
    );
  }
  return secret;
}
