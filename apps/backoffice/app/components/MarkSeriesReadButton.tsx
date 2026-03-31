"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { useTranslation } from "../../lib/i18n/context";

interface MarkSeriesReadButtonProps {
  seriesId: string;
  seriesName: string;
  bookCount: number;
  booksReadCount: number;
  compact?: boolean;
}

export function MarkSeriesReadButton({ seriesId, seriesName, bookCount, booksReadCount, compact }: MarkSeriesReadButtonProps) {
  const { t } = useTranslation();
  const [loading, setLoading] = useState(false);
  const router = useRouter();

  const allRead = bookCount > 0 && booksReadCount >= bookCount;
  const targetStatus = allRead ? "unread" : "read";
  const label = allRead
    ? t(compact ? "markRead.unread" as any : "markRead.markUnread")
    : t(compact ? "markRead.read" as any : "markRead.markAllRead");

  const handleClick = async (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setLoading(true);
    try {
      const res = await fetch("/api/series/mark-read", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ series: seriesId, status: targetStatus }),
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({ error: res.statusText }));
        console.error("Failed to mark series:", body.error);
      }
      router.refresh();
    } catch (err) {
      console.error("Failed to mark series:", err);
    } finally {
      setLoading(false);
    }
  };

  return (
    <button
      onClick={handleClick}
      disabled={loading}
      title={label}
      className={`inline-flex items-center gap-1.5 transition-colors disabled:opacity-50 ${
        compact
          ? `px-1.5 py-1 rounded-md text-xs font-medium ${allRead
              ? "text-green-600 hover:bg-green-500/20"
              : "text-muted-foreground hover:text-foreground hover:bg-accent"
            }`
          : `px-3 py-1.5 rounded-lg border text-sm font-medium ${allRead
              ? "border-green-500/30 bg-green-500/10 text-green-600 hover:bg-green-500/20"
              : "border-border bg-card text-muted-foreground hover:text-foreground hover:border-primary"
            }`
      }`}
    >
      {loading ? (
        <svg className={`${compact ? "w-3.5 h-3.5" : "w-4 h-4"} animate-spin`} fill="none" viewBox="0 0 24 24">
          <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
          <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
        </svg>
      ) : allRead ? (
        <>
          <svg className={`${compact ? "w-3.5 h-3.5" : "w-4 h-4"}`} fill="none" stroke="currentColor" viewBox="0 0 24 24" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M9 15 3 9m0 0 6-6M3 9h12a6 6 0 0 1 0 12h-3" />
          </svg>
          {label}
        </>
      ) : (
        <>
          <svg className={`${compact ? "w-3.5 h-3.5" : "w-4 h-4"}`} fill="none" stroke="currentColor" viewBox="0 0 24 24" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M9 12.75 11.25 15 15 9.75M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0z" />
          </svg>
          {label}
        </>
      )}
    </button>
  );
}
