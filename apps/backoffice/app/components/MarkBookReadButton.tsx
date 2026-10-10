"use client";

import { useTranslation } from "@/lib/i18n/context";
import { markReadClassName, useMarkRead } from "@/hooks/useMarkRead";
import { MarkReadButton } from "./MarkReadButton";

interface MarkBookReadButtonProps {
  bookId: string;
  currentStatus: string;
  compact?: boolean;
}

export function MarkBookReadButton({ bookId, currentStatus, compact }: MarkBookReadButtonProps) {
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
      completed={isRead}
      className={markReadClassName(isRead, compact)}
      onClick={handleClick}
    />
  );
}
