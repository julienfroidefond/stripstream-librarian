import { NextResponse } from "next/server";
import { ACTIVE_USER_COOKIE } from "@/lib/auth/activeUser";
import { SESSION_COOKIE } from "@/lib/auth/session";

export async function POST() {
  const response = NextResponse.json({ success: true });
  response.cookies.delete(SESSION_COOKIE);
  // The reader selection must not outlive the admin session.
  response.cookies.delete(ACTIVE_USER_COOKIE);
  return response;
}
