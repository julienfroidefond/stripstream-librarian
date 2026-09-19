"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { useTranslation } from "../../lib/i18n/context";
import { MarkReadButton } from "./MarkReadButton";

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

  const className = compact
    ? `px-1.5 py-1 rounded-md text-xs font-medium ${isRead
        ? "text-muted-foreground hover:text-foreground hover:bg-accent"
        : "text-green-600 hover:bg-green-500/20"
      }`
    : `px-3 py-1.5 rounded-lg border text-sm font-medium ${isRead
        ? "border-border bg-card text-muted-foreground hover:text-foreground hover:border-primary"
        : "border-green-500/30 bg-green-500/10 text-green-600 hover:bg-green-500/20"
      }`;

  return (
    <MarkReadButton
      label={label}
      loading={loading}
      compact={compact}
      completed={isRead}
      className={className}
      onClick={handleClick}
    />
  );
}
