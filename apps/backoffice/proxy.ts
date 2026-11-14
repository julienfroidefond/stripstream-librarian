import { NextRequest, NextResponse } from "next/server";
import { jwtVerify } from "jose";
import { readSessionSecret } from "./lib/auth/secret";
import { SESSION_COOKIE } from "./lib/auth/session";

/** Unauthenticated response: 401 JSON for API routes, redirect to /login otherwise. */
function denied(req: NextRequest, pathname: string): NextResponse {
  if (pathname.startsWith("/api/")) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }
  const loginUrl = new URL("/login", req.url);
  loginUrl.searchParams.set("from", pathname);
  return NextResponse.redirect(loginUrl);
}

/** Paths where filter persistence is active */
const FILTER_PATHS = ["/series", "/books", "/authors"];

/** Convert a basePath to a cookie name: /series → filters_series */
function filterCookieName(basePath: string): string {
  return `filters_${basePath.replace(/^\//, "").replace(/\//g, "_")}`;
}

export async function proxy(req: NextRequest) {
  const { pathname, searchParams } = req.nextUrl;

  // Skip auth for login page and auth API routes
  if (pathname.startsWith("/login") || pathname.startsWith("/api/auth")) {
    return NextResponse.next();
  }

  // Fail closed: without a usable secret no session can be trusted.
  const secret = readSessionSecret();
  if (!secret) {
    console.error("SESSION_SECRET is missing or shorter than 32 characters: refusing all sessions");
    return denied(req, pathname);
  }

  const token = req.cookies.get(SESSION_COOKIE)?.value;
  if (token) {
    try {
      await jwtVerify(token, secret);
    } catch {
      // Token invalid or expired — redirect to login
      const loginUrl = new URL("/login", req.url);
      loginUrl.searchParams.set("from", pathname);
      return NextResponse.redirect(loginUrl);
    }

    // Restore saved filters from cookie for filter pages
    if (FILTER_PATHS.includes(pathname)) {
      const nonPaginationParams = Array.from(searchParams.entries()).filter(
        ([key]) => key !== "page" && key !== "limit"
      );
      if (nonPaginationParams.length === 0) {
        const cookieName = filterCookieName(pathname);
        const cookie = req.cookies.get(cookieName);
        if (cookie?.value) {
          try {
            const filters: Record<string, string> = JSON.parse(cookie.value);
            const entries = Object.entries(filters).filter(([, v]) => v && v.trim());
            if (entries.length > 0) {
              const url = req.nextUrl.clone();
              for (const [key, value] of entries) {
                url.searchParams.set(key, value);
              }
              return NextResponse.redirect(url);
            }
          } catch {}
        }
      }
    }

    return NextResponse.next();
  }

  const loginUrl = new URL("/login", req.url);
  loginUrl.searchParams.set("from", pathname);
  return NextResponse.redirect(loginUrl);
}

export const config = {
  matcher: [
    "/((?!_next/static|_next/image|favicon\\.ico|favicon\\.png|logo\\.webp|logo-dark\\.webp|logo-mono\\.webp|logo\\.png|logostrip\\.png|manifest\\.json|apple-touch-icon.*\\.png|icons/|splash/|.*\\.svg).*)",
  ],
};
