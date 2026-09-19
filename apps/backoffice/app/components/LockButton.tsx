"use client";

import { Icon } from "./ui";
import { useTranslation } from "../../lib/i18n/context";

export function LockButton({
  locked,
  onToggle,
  disabled,
}: {
  locked: boolean;
  onToggle: () => void;
  disabled?: boolean;
}) {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      onClick={onToggle}
      disabled={disabled}
      className={`p-1 rounded transition-colors ${
        locked
          ? "text-amber-500 hover:text-amber-600"
          : "text-muted-foreground/40 hover:text-muted-foreground"
      }`}
      title={locked ? t("editBook.lockedField") : t("editBook.clickToLock")}
    >
      {locked ? (
        <Icon name="lock" size="sm" />
      ) : (
        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 11V7a4 4 0 118 0m-4 8v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2z" />
        </svg>
      )}
    </button>
  );
}
