"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { Button } from "./ui";
import { useTranslation } from "../../lib/i18n/context";

interface MarkBookReadButtonProps {
  bookId: string;
  currentStatus: string;
  compact?: boolean;
}

export function MarkBookReadButton({ bookId, currentStatus, compact }: MarkBookReadButtonProps) {
  const { t } = useTranslation();
  const [loading, setLoading] = useState(false);
  const router = useRouter();

  const isRead = currentStatus === "read";
  const targetStatus = isRead ? "unread" : "read";
  const label = isRead
    ? t(compact ? "markRead.unread" as any : "markRead.markUnread")
    : t(compact ? "markRead.read" as any : "markRead.markAsRead");

  const handleClick = async (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setLoading(true);
    try {
      const res = await fetch(`/api/books/${bookId}/progress`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ status: targetStatus }),
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({ error: res.statusText }));
        console.error("Failed to update reading progress:", body.error);
      }
      router.refresh();
    } catch (err) {
      console.error("Failed to update reading progress:", err);
    } finally {
      setLoading(false);
    }
  };

  return (
    <button
      type="button"
      onClick={handleClick}
      disabled={loading}
      title={label}
      className={`inline-flex items-center gap-1.5 transition-colors disabled:opacity-50 ${
        compact
          ? `px-1.5 py-1 rounded-md text-xs font-medium ${isRead
              ? "text-muted-foreground hover:text-foreground hover:bg-accent"
              : "text-green-600 hover:bg-green-500/20"
            }`
          : `px-3 py-1.5 rounded-lg border text-sm font-medium ${isRead
              ? "border-border bg-card text-muted-foreground hover:text-foreground hover:border-primary"
              : "border-green-500/30 bg-green-500/10 text-green-600 hover:bg-green-500/20"
            }`
      }`}
    >
      {loading ? (
        <svg className={`${compact ? "w-3.5 h-3.5" : "w-4 h-4"} animate-spin`} fill="none" viewBox="0 0 24 24">
          <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
          <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
        </svg>
      ) : isRead ? (
        <svg className={`${compact ? "w-3.5 h-3.5" : "w-4 h-4"}`} fill="none" stroke="currentColor" viewBox="0 0 24 24" strokeWidth={2}>
          <path strokeLinecap="round" strokeLinejoin="round" d="M9 15 3 9m0 0 6-6M3 9h12a6 6 0 0 1 0 12h-3" />
        </svg>
      ) : (
        <svg className={`${compact ? "w-3.5 h-3.5" : "w-4 h-4"}`} fill="none" stroke="currentColor" viewBox="0 0 24 24" strokeWidth={2}>
          <path strokeLinecap="round" strokeLinejoin="round" d="M9 12.75 11.25 15 15 9.75M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0z" />
        </svg>
      )}
      {label}
    </button>
  );
}
