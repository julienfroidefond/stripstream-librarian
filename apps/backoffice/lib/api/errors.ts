/**
 * Typed error raised by {@link apiFetch} when the upstream API answers with a
 * non-2xx status. Callers can branch on `error.status` instead of sniffing the
 * message text.
 */
export class ApiError extends Error {
  constructor(
    readonly status: number,
    message: string,
    /** Upstream `error` field, when the response body exposed one. */
    readonly detail?: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}
