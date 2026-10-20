"use client";

import { useTransition } from "react";
import { useRouter } from "next/navigation";
import {
  refreshBooksAction,
  refreshSeriesAction,
  refreshSeriesByIdAction,
} from "../actions/cache";
import { useTranslation } from "@/lib/i18n/context";
import { RefreshIcon } from "./RefreshIcon";
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

  const label = pending ? t("common.refreshing") : t("common.refresh");

  return (
    <Button
      type="button"
      variant="outline"
      size="sm"
      onClick={onClick}
      disabled={pending}
      className={className}
      title={label}
      aria-label={label}
    >
      <RefreshIcon spinning={pending} className="sm:mr-1.5" />
      <span className="hidden sm:inline">{label}</span>
    </Button>
  );
}
