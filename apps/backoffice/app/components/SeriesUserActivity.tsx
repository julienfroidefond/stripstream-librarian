"use client";

import type { SeriesUserActivityDto } from "@/lib/api";
import { useTranslation } from "@/lib/i18n/context";

import RatingStars from "./RatingStars";

/** Read-only ratings of every reader, shown on a series page in admin mode. */
export function SeriesUserRatings({ items }: { items: SeriesUserActivityDto[] }) {
  const { t } = useTranslation();
  const rated = items.filter((item) => item.rating != null);

  if (rated.length === 0) return null;

  return (
    <div className="flex flex-wrap items-center gap-2">
      <span className="text-xs font-medium text-muted-foreground">{t("series.userRatings")}</span>
      {rated.map((item) => (
        <span
          key={item.user_id}
          className="inline-flex items-center gap-1.5 rounded-lg border border-border/60 bg-muted/60 px-2 py-1 text-xs"
        >
          <span className="max-w-24 truncate font-medium text-foreground">{item.username}</span>
          <RatingStars value={item.rating} readOnly size="sm" />
          <span className="tabular-nums text-muted-foreground">
            {(item.rating! / 2).toFixed(1)}
          </span>
        </span>
      ))}
    </div>
  );
}

/** Read-only progress of every reader, shown on a series page in admin mode. */
export function SeriesUserReading({ items }: { items: SeriesUserActivityDto[] }) {
  const { t } = useTranslation();
  const active = items.filter(
    (item) => item.books_read > 0 || item.books_reading > 0
  );

  if (active.length === 0) return null;

  return (
    <div className="flex flex-wrap items-center gap-2">
      <span className="text-xs font-medium text-muted-foreground">{t("series.userReading")}</span>
      {active.map((item) => (
        <span
          key={item.user_id}
          className="inline-flex items-center gap-1.5 rounded-full border border-border/60 bg-muted/60 px-2 py-0.5 text-2xs"
        >
          <span className="max-w-24 truncate font-medium text-foreground">{item.username}</span>
          <span className="tabular-nums text-success" title={t("settings.readingOverview.booksRead")}>
            {item.books_read}
          </span>
          {item.books_reading > 0 && (
            <span className="tabular-nums text-warning" title={t("settings.readingOverview.booksReading")}>
              +{item.books_reading}
            </span>
          )}
          <span className="tabular-nums text-muted-foreground">/{item.books_total}</span>
        </span>
      ))}
    </div>
  );
}
