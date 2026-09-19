"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { useTranslation } from "../../lib/i18n/context";
import { MarkReadButton } from "./MarkReadButton";

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

  const className = compact
    ? `px-1.5 py-1 rounded-md text-xs font-medium ${allRead
        ? "text-green-600 hover:bg-green-500/20"
        : "text-muted-foreground hover:text-foreground hover:bg-accent"
      }`
    : `px-3 py-1.5 rounded-lg border text-sm font-medium ${allRead
        ? "border-green-500/30 bg-green-500/10 text-green-600 hover:bg-green-500/20"
        : "border-border bg-card text-muted-foreground hover:text-foreground hover:border-primary"
      }`;

  return (
    <MarkReadButton
      label={label}
      loading={loading}
      compact={compact}
      completed={allRead}
      className={className}
      onClick={handleClick}
    />
  );
}
