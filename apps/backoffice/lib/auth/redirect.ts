/**
 * Post-login redirect target. Only a same-origin relative path is accepted
 * (starts with `/`, not `//` nor `/\`, no control characters); anything else
 * (absolute URL, `javascript:`, protocol-relative) falls back to `/`.
 */
export function safeRedirectPath(from: string | null | undefined): string {
  if (!from) return "/";
  if (!from.startsWith("/")) return "/";
  if (from.startsWith("//") || from.startsWith("/\\")) return "/";
  // Browsers strip tabs/newlines and treat `\` as `/`, which would reopen the
  // protocol-relative hole (`/\t/evil.com`).
  if (/[\u0000-\u001f\u007f\\]/.test(from)) return "/";
  return from;
}
