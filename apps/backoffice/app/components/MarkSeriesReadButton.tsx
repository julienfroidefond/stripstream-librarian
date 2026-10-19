"use client";

import { useTranslation } from "@/lib/i18n/context";
import { inlineReadClassName, markReadClassName, overlayReadClassName, useMarkRead } from "@/hooks/useMarkRead";
import { MarkReadButton } from "./MarkReadButton";

interface MarkSeriesReadButtonProps {
  seriesId: string;
  seriesName: string;
  bookCount: number;
  booksReadCount: number;
  compact?: boolean;
  iconOnly?: boolean;
  /** Icon-only look: dark pill over a cover, neutral square in a list row. */
  variant?: "overlay" | "inline";
}

export function MarkSeriesReadButton({ seriesId, seriesName, bookCount, booksReadCount, compact, iconOnly, variant = "overlay" }: MarkSeriesReadButtonProps) {
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
      iconOnly={iconOnly}
      completed={allRead}
      className={
        iconOnly
          ? variant === "inline"
            ? inlineReadClassName(allRead)
            : overlayReadClassName(allRead)
          : markReadClassName(allRead, compact)
      }
      onClick={handleClick}
    />
  );
}
