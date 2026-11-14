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
