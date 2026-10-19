"use client";

import { useTranslation } from "@/lib/i18n/context";
import { inlineReadClassName, markReadClassName, overlayReadClassName, useMarkRead } from "@/hooks/useMarkRead";
import { MarkReadButton } from "./MarkReadButton";

interface MarkBookReadButtonProps {
  bookId: string;
  currentStatus: string;
  compact?: boolean;
  iconOnly?: boolean;
  /** Icon-only look: dark pill over a cover, neutral square in a list row. */
  variant?: "overlay" | "inline";
}

export function MarkBookReadButton({ bookId, currentStatus, compact, iconOnly, variant = "overlay" }: MarkBookReadButtonProps) {
  const { t } = useTranslation();

  const isRead = currentStatus === "read";
  const targetStatus = isRead ? "unread" : "read";
  const label = isRead
    ? t(compact ? "markRead.unread" as any : "markRead.markUnread")
    : t(compact ? "markRead.read" as any : "markRead.markAsRead");

  const { loading, handleClick } = useMarkRead({
    url: `/api/books/${bookId}/progress`,
    method: "PATCH",
    body: { status: targetStatus },
    errorContext: "Failed to update reading progress",
  });

  return (
    <MarkReadButton
      label={label}
      loading={loading}
      compact={compact}
      iconOnly={iconOnly}
      completed={isRead}
      className={
        iconOnly
          ? variant === "inline"
            ? inlineReadClassName(isRead)
            : overlayReadClassName(isRead)
          : markReadClassName(isRead, compact)
      }
      onClick={handleClick}
    />
  );
}
