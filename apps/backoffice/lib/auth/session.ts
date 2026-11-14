import { SignJWT, jwtVerify } from "jose";
import { cookies } from "next/headers";

import { requireSessionSecret } from "./secret";

export const SESSION_COOKIE = "sl_session";

function getSecret(): Uint8Array {
  return requireSessionSecret();
}

export async function createSessionToken(): Promise<string> {
  return new SignJWT({})
    .setProtectedHeader({ alg: "HS256" })
    .setExpirationTime("7d")
    .sign(getSecret());
}

export async function verifySessionToken(token: string): Promise<boolean> {
  try {
    await jwtVerify(token, getSecret());
    return true;
  } catch {
    return false;
  }
}

export async function getSession(): Promise<boolean> {
  const cookieStore = await cookies();
  const token = cookieStore.get(SESSION_COOKIE)?.value;
  if (!token) return false;
  return verifySessionToken(token);
}
