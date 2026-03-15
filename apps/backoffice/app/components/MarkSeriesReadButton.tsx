"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";

interface MarkSeriesReadButtonProps {
  seriesName: string;
  bookCount: number;
  booksReadCount: number;
}

export function MarkSeriesReadButton({ seriesName, bookCount, booksReadCount }: MarkSeriesReadButtonProps) {
  const [loading, setLoading] = useState(false);
  const router = useRouter();

  const allRead = booksReadCount >= bookCount;
  const targetStatus = allRead ? "unread" : "read";
  const label = allRead ? "Marquer non lu" : "Tout marquer lu";

  const handleClick = async (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setLoading(true);
    try {
      const res = await fetch("/api/series/mark-read", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ series: seriesName, status: targetStatus }),
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
      className={`inline-flex items-center gap-1 text-xs px-2 py-1 rounded-full font-medium transition-colors ${
        allRead
          ? "bg-green-500/15 text-green-600 dark:text-green-400 hover:bg-green-500/25"
          : "bg-muted/50 text-muted-foreground hover:bg-primary/10 hover:text-primary"
      } disabled:opacity-50`}
    >
      {loading ? (
        <svg className="w-3.5 h-3.5 animate-spin" fill="none" viewBox="0 0 24 24">
          <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
          <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
        </svg>
      ) : allRead ? (
        <>
          <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M9 15 3 9m0 0 6-6M3 9h12a6 6 0 0 1 0 12h-3" />
          </svg>
          {label}
        </>
      ) : (
        <>
          <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M9 12.75 11.25 15 15 9.75M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0z" />
          </svg>
          {label}
        </>
      )}
    </button>
  );
}
