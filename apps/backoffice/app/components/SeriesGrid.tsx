import { Fragment } from "react";
import Image from "next/image";
import Link from "next/link";
import { getBookCoverUrl, type SeriesDto } from "@/lib/api";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";
import { MarkSeriesReadButton } from "./MarkSeriesReadButton";

interface SeriesGridProps {
  series: SeriesDto[];
  sort?: string;
  hasActiveUser: boolean;
  t: TranslateFunction;
  knownStatuses: Record<string, string>;
  showCommunityScore?: boolean;
}

export function SeriesGrid({ series, sort, hasActiveUser, t, knownStatuses, showCommunityScore = false }: SeriesGridProps) {
  return (
    <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 xl:grid-cols-6 gap-4">
      {series.map((s, index) => {
        const year = s.start_year;
        const previousYear = index > 0 ? series[index - 1].start_year : undefined;
        const showYearGroup = sort === "release_date" && year !== previousYear;

        const communityGroup = s.community_score != null ? Math.floor(s.community_score) : null;
        const prevScore = index > 0 ? series[index - 1].community_score : undefined;
        const previousCommunityGroup = prevScore != null ? Math.floor(prevScore) : (prevScore === undefined ? undefined : null);
        const showCommunityGroup = showCommunityScore && sort === "community_score" && communityGroup !== previousCommunityGroup;
        const communityGroupLabel = communityGroup != null
          ? "★".repeat(communityGroup) + "☆".repeat(Math.max(0, 5 - communityGroup)) + `  ${communityGroup}/5`
          : t("series.noCommunityScore");

        return (
          <Fragment key={s.series_id}>
            {showYearGroup && (
              <div className="col-span-full flex items-center gap-3 pt-2 first:pt-0">
                <span className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                  {year ?? t("series.noReleaseDate")}
                </span>
                <span className="h-px flex-1 bg-border/60" />
              </div>
            )}
            {showCommunityGroup && (
              <div className="col-span-full flex items-center gap-3 pt-2 first:pt-0">
                <span className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                  {communityGroupLabel}
                </span>
                <span className="h-px flex-1 bg-border/60" />
              </div>
            )}
            <div className="group">
              <div className="bg-card rounded-xl shadow-sm border border-border/60 overflow-hidden group-hover:shadow-md group-hover:-translate-y-1 transition-all duration-200">
                <Link href={`/series/${s.series_id}`} className="block">
                  <div className="aspect-[2/3] relative bg-muted/50">
                    {(s.first_book_id || s.cover_url) ? (
                      <Image
                        src={s.first_book_id ? getBookCoverUrl(s.first_book_id, s.first_book_updated_at) : s.cover_url!}
                        alt={t("books.coverOf", { name: s.name })}
                        fill
                        className={`object-cover ${s.book_count > 0 && s.books_read_count >= s.book_count ? "opacity-40" : ""}`}
                        sizes="(max-width: 640px) 50vw, (max-width: 768px) 33vw, (max-width: 1024px) 25vw, 16vw"
                      />
                    ) : (
                      <div className="w-full h-full flex items-center justify-center text-muted-foreground/30">
                        <svg className="w-12 h-12" fill="none" stroke="currentColor" viewBox="0 0 24 24" strokeWidth={1.5}>
                          <path strokeLinecap="round" strokeLinejoin="round" d="M12 6.042A8.967 8.967 0 0 0 6 3.75c-1.052 0-2.062.18-3 .512v14.25A8.987 8.987 0 0 1 6 18c2.305 0 4.408.867 6 2.292m0-14.25a8.966 8.966 0 0 1 6-2.292c1.052 0 2.062.18 3 .512v14.25A8.987 8.987 0 0 0 18 18a8.967 8.967 0 0 0-6 2.292m0-14.25v14.25" />
                        </svg>
                      </div>
                    )}
                    {(s.series_status || (s.missing_count != null && s.missing_count > 0)) && (
                      <div className="absolute top-1.5 right-1.5 flex items-center gap-1">
                        {s.series_status && (
                          <span className={`text-[10px] px-1.5 py-0.5 rounded-full font-medium ${
                            s.series_status === "ongoing" ? "bg-blue-500 text-white" :
                            s.series_status === "ended" ? "bg-green-500 text-white" :
                            s.series_status === "hiatus" ? "bg-amber-500 text-white" :
                            s.series_status === "cancelled" ? "bg-red-500 text-white" :
                            "bg-muted text-muted-foreground"
                          }`}>
                            {knownStatuses[s.series_status] || s.series_status}
                          </span>
                        )}
                        {s.missing_count != null && s.missing_count > 0 && (
                          <span className="inline-flex items-center gap-0.5 text-[10px] px-1.5 py-0.5 rounded-full font-bold bg-yellow-500 text-white" title={t("series.missingCount", { count: String(s.missing_count), plural: s.missing_count > 1 ? "s" : "" })}>
                            <svg className="w-2.5 h-2.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2.5}><path d="M12 9v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" strokeLinecap="round" strokeLinejoin="round"/></svg>
                            {s.missing_count}
                          </span>
                        )}
                      </div>
                    )}
                  </div>
                </Link>
                <div className="px-2 py-1.5">
                  <Link href={`/series/${s.series_id}`}>
                    <h3 className="font-medium text-foreground truncate text-xs hover:text-primary transition-colors" title={s.name}>
                      {s.name === "unclassified" ? t("books.unclassified") : s.name}
                    </h3>
                  </Link>
                  <div className="flex items-center justify-between mt-0.5">
                    <p className="text-[11px] text-muted-foreground">
                      {t("series.readCount", { read: String(s.books_read_count), total: String(s.book_count), plural: s.book_count !== 1 ? "s" : "" })}
                    </p>
                    {hasActiveUser && (
                      <MarkSeriesReadButton
                        seriesId={s.series_id}
                        seriesName={s.name}
                        bookCount={s.book_count}
                        booksReadCount={s.books_read_count}
                        compact
                      />
                    )}
                  </div>
                </div>
              </div>
            </div>
          </Fragment>
        );
      })}
    </div>
  );
}
