"use client";

import { useTransition } from "react";
import { useRouter } from "next/navigation";
import {
  refreshBooksAction,
  refreshSeriesAction,
  refreshSeriesByIdAction,
} from "../actions/cache";
import { useTranslation } from "@/lib/i18n/context";
import { Button } from "./ui/Button";

type Target = "books" | "series" | "series-detail" | "book-detail";

interface Props {
  target: Target;
  seriesId?: string;
  className?: string;
  children?: (onClick: () => void, pending: boolean) => React.ReactNode;
}

export function RefreshButton({ target, seriesId, className = "", children }: Props) {
  const { t } = useTranslation();
  const router = useRouter();
  const [pending, startTransition] = useTransition();

  const onClick = () => {
    startTransition(async () => {
      if (target === "books") await refreshBooksAction();
      else if (target === "series") await refreshSeriesAction();
      else if (target === "series-detail" && seriesId) await refreshSeriesByIdAction(seriesId);
      router.refresh();
    });
  };

  if (children) {
    return <>{children(onClick, pending)}</>;
  }

  return (
    <Button
      type="button"
      variant="outline"
      size="sm"
      onClick={onClick}
      disabled={pending}
      className={className}
      title={t("common.refresh")}
    >
      <svg
        className={`w-4 h-4 sm:mr-1.5 ${pending ? "animate-spin" : ""}`}
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth={2}
        strokeLinecap="round"
        strokeLinejoin="round"
      >
        <path d="M3 12a9 9 0 0 1 15-6.7L21 8" />
        <path d="M21 3v5h-5" />
        <path d="M21 12a9 9 0 0 1-15 6.7L3 16" />
        <path d="M3 21v-5h5" />
      </svg>
      <span className="hidden sm:inline">{pending ? t("common.refreshing") : t("common.refresh")}</span>
    </Button>
  );
}
