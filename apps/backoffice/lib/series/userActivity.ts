import type { SeriesUserActivityDto } from "@/lib/api";

/** Reader activity whose half-star rating is known to be present. */
export type RatedSeriesUserActivity = SeriesUserActivityDto & { rating: number };

function hasRating(item: SeriesUserActivityDto): item is RatedSeriesUserActivity {
  return item.rating != null;
}

/** Readers who rated the series. */
export function ratedActivity(
  items: SeriesUserActivityDto[]
): RatedSeriesUserActivity[] {
  return items.filter(hasRating);
}

/** Readers who started the series (read or currently reading). */
export function readingActivity(
  items: SeriesUserActivityDto[]
): SeriesUserActivityDto[] {
  return items.filter(
    (item) => item.books_read > 0 || item.books_reading > 0
  );
}
