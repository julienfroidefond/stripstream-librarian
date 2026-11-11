"use client";

/**
 * Catalog of fields the metadata sync can write, fetched once by the server
 * layout and shared with every lock UI. Keeping it in a context avoids both a
 * client fetch per form and a second hardcoded list of lockable keys.
 */

import { createContext, useContext, useMemo, type ReactNode } from "react";
import type { MetadataFieldCatalogDto } from "@/lib/api";

export type MetadataFieldsTarget = "series" | "books";

const MetadataFieldsContext = createContext<MetadataFieldCatalogDto | null>(null);

export function MetadataFieldsProvider({
  catalog,
  children,
}: {
  catalog: MetadataFieldCatalogDto | null;
  children: ReactNode;
}) {
  return (
    <MetadataFieldsContext.Provider value={catalog}>
      {children}
    </MetadataFieldsContext.Provider>
  );
}

/**
 * Lockable field keys for a catalog target. Returns `null` when the catalog is
 * unavailable: callers must then keep the locks they already have instead of
 * dropping them, since the API sanitizes writes anyway.
 */
export function useLockableFields(target: MetadataFieldsTarget): readonly string[] | null {
  const catalog = useContext(MetadataFieldsContext);

  return useMemo(() => {
    if (!catalog) return null;
    return catalog[target]
      .filter((field) => field.lockable)
      .map((field) => field.key);
  }, [catalog, target]);
}
