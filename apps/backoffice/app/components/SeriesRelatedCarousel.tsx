import Link from "next/link";
import Image from "next/image";
import type { RelatedSeriesDto } from "@/lib/api";
import { getBookCoverUrl } from "@/lib/api";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";

const REASON_KEYS: Record<string, string> = {
  same_author: "series.relatedSameAuthor",
  same_genre: "series.relatedSameGenre",
  same_publisher: "series.relatedSamePublisher",
};

const REASON_COLORS: Record<string, string> = {
  same_author: "bg-primary/15 text-primary",
  same_genre: "bg-success/15 text-success",
  same_publisher: "bg-warning/15 text-warning",
};

export function SeriesRelatedCarousel({
  items,
  t,
}: {
  items: RelatedSeriesDto[];
  t: TranslateFunction;
}) {
  if (items.length === 0) return null;

  return (
    <div className="mt-10">
      <h2 className="text-sm font-semibold text-muted-foreground uppercase tracking-wider mb-4 flex items-center gap-3">
        <span className="h-px flex-1 bg-border" />
        <span>{t("series.relatedTitle")}</span>
        <span className="h-px flex-1 bg-border" />
      </h2>
      <div className="flex gap-5 overflow-x-auto pb-4 -mx-1 px-1" style={{ scrollbarWidth: "none" }}>
        {items.map((series) => {
          const coverUrl = series.first_book_id
            ? getBookCoverUrl(series.first_book_id, series.first_book_updated_at ?? undefined)
            : series.cover_url;
          return (
            <Link
              key={series.series_id}
              href={`/series/${series.series_id}`}
              className="flex-none w-44 group"
            >
              {/* Cover */}
              <div className="w-44 aspect-[2/3] relative rounded-xl overflow-hidden border border-border/60 shadow-md mb-3 group-hover:shadow-lg group-hover:border-primary/40 transition-all duration-200">
                {coverUrl ? (
                  series.first_book_id ? (
                    <Image
                      src={coverUrl}
                      alt={series.name}
                      fill
                      className="object-cover group-hover:scale-105 transition-transform duration-300"
                      sizes="176px"
                    />
                  ) : (
                    /* eslint-disable-next-line @next/next/no-img-element */
                    <img
                      src={coverUrl}
                      alt={series.name}
                      className="w-full h-full object-cover group-hover:scale-105 transition-transform duration-300"
                    />
                  )
                ) : (
                  <div className="w-full h-full bg-muted flex items-center justify-center">
                    <svg className="w-10 h-10 text-muted-foreground/30" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
                    </svg>
                  </div>
                )}

                {/* Reason badges overlaid at bottom */}
                <div className="absolute bottom-0 inset-x-0 p-2 bg-gradient-to-t from-black/70 to-transparent flex flex-wrap gap-1">
                  {series.match_reasons.map((reason) => (
                    <span
                      key={reason}
                      className={`text-[10px] px-1.5 py-0.5 rounded-full font-medium backdrop-blur-sm ${REASON_COLORS[reason] ?? "bg-muted/80 text-muted-foreground"}`}
                    >
                      {t(REASON_KEYS[reason] as any)}
                    </span>
                  ))}
                </div>
              </div>

              {/* Title & meta */}
              <p className="text-sm font-semibold text-foreground leading-tight line-clamp-2 group-hover:text-primary transition-colors mb-1">
                {series.name}
              </p>
              <p className="text-xs text-muted-foreground">
                {series.book_count} {series.book_count === 1 ? "vol." : "vols."}
              </p>
            </Link>
          );
        })}
      </div>
    </div>
  );
}
