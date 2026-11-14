"use client";

import { createContext, useContext, type ReactNode } from "react";

import type { AcquisitionReadiness } from "@/lib/acquisition/readiness";

const AcquisitionReadinessContext = createContext<AcquisitionReadiness | null>(null);

/**
 * Rend la readiness résolue côté serveur aux composants clients (menus de
 * cartes, modales) sans refetch de settings au montage.
 */
export function AcquisitionReadinessProvider({
  readiness,
  children,
}: {
  readiness: AcquisitionReadiness;
  children: ReactNode;
}) {
  return (
    <AcquisitionReadinessContext.Provider value={readiness}>
      {children}
    </AcquisitionReadinessContext.Provider>
  );
}

export function useAcquisitionReadiness(): AcquisitionReadiness | null {
  return useContext(AcquisitionReadinessContext);
}
