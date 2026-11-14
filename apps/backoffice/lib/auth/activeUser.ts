/**
 * Reader selected in the backoffice (`as_user_id` cookie).
 *
 * Server-only. The dynamic import keeps the API barrel importable outside a
 * Next.js request context (unit tests, route handlers).
 */

export const ACTIVE_USER_COOKIE = "as_user_id";

export type ActiveUser = {
  /** Selected reader UUID, or null in admin mode. */
  userId: string | null;
  hasActiveUser: boolean;
};

export async function getActiveUser(): Promise<ActiveUser> {
  const { cookies } = await import("next/headers");
  const userId = (await cookies()).get(ACTIVE_USER_COOKIE)?.value || null;
  return { userId, hasActiveUser: userId !== null };
}

/** Upstream 400 messages meaning the `X-As-User` header names no usable reader. */
const STALE_ACTIVE_USER_MESSAGES = [
  "X-As-User references an unknown user",
  "X-As-User must be a valid user id",
];

/** True when the API rejected the request because the selected reader is gone. */
export function isStaleActiveUserError(status: number, detail: string | undefined): boolean {
  return status === 400 && !!detail && STALE_ACTIVE_USER_MESSAGES.some((m) => detail.includes(m));
}

/**
 * Drops the reader selection (back to admin mode). Only effective where
 * cookies are writable (route handlers, server actions); a no-op during a
 * Server Component render.
 */
export async function clearActiveUser(): Promise<void> {
  try {
    const { cookies } = await import("next/headers");
    (await cookies()).delete(ACTIVE_USER_COOKIE);
  } catch {
    // Read-only cookie store (render) or outside a request.
  }
}

/** Drops the reader selection when it points to `userId` (e.g. a deleted reader). */
export async function clearActiveUserIf(userId: string): Promise<boolean> {
  const { userId: active } = await getActiveUser();
  if (active !== userId) return false;
  await clearActiveUser();
  return true;
}
