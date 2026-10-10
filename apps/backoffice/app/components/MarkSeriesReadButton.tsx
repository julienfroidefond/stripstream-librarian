"use client";

import { useTranslation } from "@/lib/i18n/context";
import { markReadClassName, useMarkRead } from "@/hooks/useMarkRead";
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

  const allRead = bookCount > 0 && booksReadCount >= bookCount;
  const targetStatus = allRead ? "unread" : "read";
  const label = allRead
    ? t(compact ? "markRead.unread" as any : "markRead.markUnread")
    : t(compact ? "markRead.read" as any : "markRead.markAllRead");

  const { loading, handleClick } = useMarkRead({
    url: "/api/series/mark-read",
    method: "POST",
    body: { series: seriesId, status: targetStatus },
    errorContext: "Failed to mark series",
  });

  return (
    <MarkReadButton
      label={label}
      loading={loading}
      compact={compact}
      completed={allRead}
      className={markReadClassName(allRead, compact)}
      onClick={handleClick}
    />
  );
}
