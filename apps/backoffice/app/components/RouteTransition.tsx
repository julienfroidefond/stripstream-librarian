"use client";

import { useState, type ReactNode } from "react";
import { usePathname } from "next/navigation";

/**
 * Animates the page content in on every pathname change.
 *
 * The wrapper is keyed on the pathname, so a navigation to a new route
 * remounts it with the `animate-route-in` class already applied (no flash of
 * un-animated content). Query-string-only updates (`?q=`, `?page=`, …) keep
 * the same pathname and therefore do **not** remount or replay the animation,
 * which keeps list filtering and pagination calm.
 */
export function RouteTransition({ children }: { children: ReactNode }) {
  const pathname = usePathname();
  const [previous, setPrevious] = useState(pathname);
  const [generation, setGeneration] = useState(0);

  // Adjusting state during render (React's documented pattern): the new
  // pathname is committed on the extra render pass, so the keyed wrapper
  // remounts with the animation class already applied — no flash.
  if (previous !== pathname) {
    setPrevious(pathname);
    setGeneration((value) => value + 1);
  }

  return (
    <div key={generation} className="animate-route-in">
      {children}
    </div>
  );
}
