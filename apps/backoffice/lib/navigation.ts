/**
 * Shared navigation types.
 *
 * `NavHref` is the single source of truth for the routes reachable from the
 * main navigation. It is used by the desktop nav (`layout.tsx`), the mobile
 * drawer (`MobileNav`) and the grouped dropdown (`NavDropdown`) so a typo in a
 * route is a compile error rather than a dead link.
 */
export type NavHref =
  | "/"
  | "/books"
  | "/series"
  | "/authors"
  | "/libraries"
  | "/discovery"
  | "/jobs"
  | "/tokens"
  | "/settings"
  | "/downloads"
  | "/genres"
  | "/reading-lists"
  | "/metadata";
