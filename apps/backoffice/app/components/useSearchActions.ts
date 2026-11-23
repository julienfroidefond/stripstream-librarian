"use client";

import { useState, useSyncExternalStore } from "react";

import { toast } from "./ui";
import { browserFailureMessage } from "@/lib/api/browser";
import {
  createSearchActionStore,
  type SearchActionContext,
  type SearchActionStore,
  type SearchResultAction,
  type SearchResultStatus,
} from "@/lib/downloads/searchActions";

/** What a search modal consumes: one overlay read and one fire-and-forget write. */
export interface SearchActions {
  /** Whether the row has a queued or in-flight write. */
  pending(key: string): boolean;
  /** The optimistic badge of the row, or `null` when the server status shows. */
  status(key: string): SearchResultStatus | null;
  /** Run one action; the store never rejects (failures reach the toast). */
  run(action: SearchResultAction, key: string, context?: SearchActionContext): Promise<boolean>;
  /** Drop the overlays a fresh server read supersedes. */
  clear(keys: readonly string[]): void;
  /** Drop every overlay when a new search replaces the rows. */
  reset(): void;
}

/**
 * One store per mounted modal. The store outlives every render and wires its
 * failure policy to the global toast: a rejected dismiss is surfaced like any
 * other write, never swallowed. Subscribing here is what re-renders the rows
 * when `pending`/`status` change.
 */
function useSearchActionStore(): SearchActionStore {
  const [store] = useState(() =>
    createSearchActionStore({
      onError(error) {
        toast(browserFailureMessage(error), "error");
      },
    }),
  );

  useSyncExternalStore(store.subscribe, store.version, store.version);

  // No dispose on unmount: the store holds no timer, React drops its
  // subscription, and disposing would break the StrictMode remount.
  return store;
}

/** Write actions of the Telegram live-search modal. */
export function useTelegramSearchActions(): SearchActions {
  return useSearchActionStore();
}

/** Write actions of the DDL provider search modal. */
export function useDdlSearchActions(): SearchActions {
  return useSearchActionStore();
}
