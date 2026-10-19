"use client";

import { useState, type MouseEvent } from "react";
import { useRouter } from "next/navigation";

import { iconActionClassName } from "@/app/components/ui/iconAction";

export interface UseMarkReadOptions {
  /** Target endpoint, e.g. `/api/books/<id>/progress`. */
  url: string;
  method: "POST" | "PATCH" | "PUT";
  body: Record<string, unknown>;
  /** Prefix used when logging failures, e.g. `Failed to update reading progress`. */
  errorContext: string;
}

export interface UseMarkReadResult {
  loading: boolean;
  handleClick: (e: MouseEvent) => Promise<void>;
}

/**
 * Shared "mark as read/unread" click handler: prevents navigation, posts the
 * status, logs failures and refreshes the current route once done.
 */
export function useMarkRead({ url, method, body, errorContext }: UseMarkReadOptions): UseMarkReadResult {
  const [loading, setLoading] = useState(false);
  const router = useRouter();

  const handleClick = async (e: MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setLoading(true);
    try {
      const res = await fetch(url, {
        method,
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });
      if (!res.ok) {
        const errorBody = await res.json().catch(() => ({ error: res.statusText }));
        console.error(`${errorContext}:`, errorBody.error);
      }
      router.refresh();
    } catch (err) {
      console.error(`${errorContext}:`, err);
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
