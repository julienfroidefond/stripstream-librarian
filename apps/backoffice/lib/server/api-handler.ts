import { NextRequest, NextResponse } from "next/server";

/**
 * A Next.js App Router route handler.
 *
 * The `context` carries the route parameters (e.g. `{ params: Promise<{ id: string }> }`).
 */
export type RouteHandler<Ctx = unknown> = (
  request: NextRequest,
  context: Ctx
) => Response | Promise<Response>;

export interface WithRouteOptions {
  /** Constant error message. When set, the thrown value's message is ignored. */
  message?: string;
  /** Message used when the thrown value is not an `Error` (or has an empty message). */
  fallback?: string;
  /** HTTP status used for error responses. Defaults to `500`. */
  status?: number;
  /**
   * Maps a thrown value to a specific status, overriding `status` when it returns a number.
   * Useful to forward an upstream HTTP status carried by the error message.
   */
  statusFromError?: (error: unknown, message: string) => number | undefined;
  /** Optional side effect (e.g. logging) run before the error response is built. */
  onError?: (error: unknown, message: string) => void;
}

/**
 * Builds a JSON error response, extracting the message from the thrown value.
 *
 * Shared by {@link withRoute} and available directly for handlers with bespoke control flow.
 */
export function errorResponse(
  error: unknown,
  options: WithRouteOptions = {}
): NextResponse {
  const {
    message,
    fallback = "Internal Server Error",
    status = 500,
    statusFromError,
    onError,
  } = options;
  const resolved =
    message ?? (error instanceof Error && error.message ? error.message : fallback);
  onError?.(error, resolved);
  return NextResponse.json(
    { error: resolved },
    { status: statusFromError?.(error, resolved) ?? status }
  );
}

/**
 * Wraps a route handler so it always returns a JSON error response on failure,
 * removing the repetitive `try/catch` + `NextResponse.json({ error })` boilerplate.
 *
 * ```ts
 * export const GET = withRoute(async (_request, { params }: { params: Params }) => {
 *   const { id } = await params;
 *   return NextResponse.json(await apiFetch(`/things/${id}`));
 * }, { fallback: "Failed to fetch thing" });
 * ```
 */
export function withRoute<Ctx = unknown>(
  handler: RouteHandler<Ctx>,
  options: WithRouteOptions = {}
): (request?: NextRequest, context?: Ctx) => Promise<Response> {
  return async (request, context) => {
    try {
      return await handler(request as NextRequest, context as Ctx);
    } catch (error) {
      return errorResponse(error, options);
    }
  };
}
