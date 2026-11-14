"use client";

import { useState, type MouseEvent } from "react";
import { useRouter } from "next/navigation";

import { iconActionClassName } from "@/app/components/ui/iconAction";
import { ApiError } from "@/lib/api/errors";

export interface UseMarkReadOptions {
  /** Semantic action that performs the read/unread change. */
  action: () => Promise<unknown>;
  /** Prefix used when logging failures, e.g. `Failed to update reading progress`. */
  errorContext: string;
}

export interface UseMarkReadResult {
  loading: boolean;
  handleClick: (e: MouseEvent) => Promise<void>;
}

/**
 * Shared "mark as read/unread" click handler: prevents navigation, runs the
 * supplied action, logs failures and refreshes the current route.
 */
export function useMarkRead({ action, errorContext }: UseMarkReadOptions): UseMarkReadResult {
  const [loading, setLoading] = useState(false);
  const router = useRouter();

  const handleClick = async (e: MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setLoading(true);
    try {
      await action();
      router.refresh();
    } catch (error) {
      console.error(`${errorContext}:`, error instanceof ApiError ? error.detail ?? error.message : error);
      // An HTTP failure still changed nothing server-side, but the previous
      // implementation refreshed the route anyway; keep that so stale data is
      // re-fetched after a rejected mutation.
      if (error instanceof ApiError) router.refresh();
    } finally {
      setLoading(false);
    }
  };

  return { loading, handleClick };
}

/**
 * Shared styling for read toggles. `completed` is true when the item is already
 * read — the "done" state is highlighted in green, otherwise the control is muted.
 */
export function markReadClassName(completed: boolean, compact?: boolean): string {
  if (compact) {
    return `px-1.5 py-1 rounded-md text-xs font-medium ${
      completed
        ? "text-success hover:bg-success/20"
        : "text-muted-foreground hover:text-foreground hover:bg-accent"
    }`;
  }
  return `px-3 py-1.5 rounded-lg border text-sm font-medium ${
    completed
      ? "border-success/30 bg-success/10 text-success hover:bg-success/20"
      : "border-border bg-card text-muted-foreground hover:text-foreground hover:border-primary"
  }`;
}

/**
 * Floating control laid over a cover: a small dark pill so it stays legible on
 * any artwork, with the completed state picking up the success colour. Sized to
 * match the other shortcuts of the cover cluster (`⋯`, download, delete).
 */
export function overlayReadClassName(completed: boolean): string {
  return iconActionClassName("cover", completed ? "success" : "neutral");
}

/**
 * Icon-only control for list rows: a neutral outlined square that sits
 * naturally on a card background (unlike the dark overlay pill).
 */
export function inlineReadClassName(completed: boolean): string {
  return iconActionClassName("inline", completed ? "success" : "neutral");
}
