"use client";

import { useState, type MouseEvent } from "react";
import { useRouter } from "next/navigation";

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
        ? "text-green-600 hover:bg-green-500/20"
        : "text-muted-foreground hover:text-foreground hover:bg-accent"
    }`;
  }
  return `px-3 py-1.5 rounded-lg border text-sm font-medium ${
    completed
      ? "border-green-500/30 bg-green-500/10 text-green-600 hover:bg-green-500/20"
      : "border-border bg-card text-muted-foreground hover:text-foreground hover:border-primary"
  }`;
}
