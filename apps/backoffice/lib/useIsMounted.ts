"use client";

import { useSyncExternalStore } from "react";

const emptySubscribe = () => () => {};

/**
 * Returns `false` during SSR and the first client render, then `true` once the
 * component has mounted on the client.
 *
 * Effect-free, hydration-safe replacement for the common
 * `useState(false)` + `useEffect(() => setMounted(true), [])` guard.
 */
export function useIsMounted(): boolean {
  return useSyncExternalStore(
    emptySubscribe,
    () => true,
    () => false,
  );
}
