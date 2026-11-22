"use client";

import { useEffect, useRef, useState, useSyncExternalStore } from "react";

import { toast } from "@/app/components/ui";
import { browserApi, browserErrorMessage } from "@/lib/api/browser";
import { createGenreActionStore, type GenreWrite } from "@/lib/genres/actions";
import { useTranslation } from "@/lib/i18n/context";

export interface GenreActions {
  /** Whether the genre currently has a queued or in-flight write. */
  busy(genre: string): boolean;
  /** Run one write; resolves `false` after the error toast on failure. */
  run(write: GenreWrite, after?: () => Promise<void> | void): Promise<boolean>;
}

/**
 * The genres page's write actions. `busy(genre)` reads the store synchronously,
 * so subscribing here is what re-renders the cards and the modal.
 */
export function useGenreActions(): GenreActions {
  const { t } = useTranslation();
  // The store is created once and outlives every render, so it reads the
  // current translator through a ref instead of capturing the first one.
  const tRef = useRef(t);
  useEffect(() => {
    tRef.current = t;
  }, [t]);

  const [store] = useState(() =>
    createGenreActionStore({
      api: browserApi.genres,
      onError(error, write) {
        const fallback =
          write.type === "applySuggestion"
            ? tRef.current("genres.aiError")
            : tRef.current("common.error");
        toast(browserErrorMessage(error, fallback), "error");
      },
    }),
  );

  useSyncExternalStore(store.subscribe, store.version, store.version);

  return { busy: store.busy, run: store.run };
}
