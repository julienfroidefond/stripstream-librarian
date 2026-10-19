import { Fragment } from "react";
import Link from "next/link";

import { getBookCoverUrl, type SeriesDto } from "@/lib/api";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";
import { JacketGrid } from "./motion/JacketGrid";
import { SeriesCardActions } from "./CardActions";
import { providerLabel } from "./ProviderIcon";
import { Icon } from "./ui/Icon";
import {
  CoverBadge,
  CoverCard,
  CoverImage,
  CoverProgress,
  CoverRow,
  CoverScrim,
  type CoverBadgeTone,
} from "./ui/CoverCard";

interface SeriesGridProps {
  series: SeriesDto[];
  sort?: string;
  hasActiveUser: boolean;
  t: TranslateFunction;
  knownStatuses: Record<string, string>;
  showCommunityScore?: boolean;
}

const seriesStatusTones: Record<string, CoverBadgeTone> = {
  ongoing: "info",
  ended: "success",
  hiatus: "warning",
  cancelled: "error",
};

/** Cover URL of a series: first book thumbnail when available, else the stored cover. */
export function seriesCoverUrl(series: SeriesDto): string {
  if (series.first_book_id) {
    return getBookCoverUrl(series.first_book_id, series.first_book_updated_at);
  }
  return series.cover_url ?? "";
}

/**
 * Values derived from a series' position in the (already sorted) list:
 * grouping headers, reading completion and status tone. Shared by the grid and
 * the list so both stay visually and behaviourally aligned.
 */
function seriesDerived(
  series: SeriesDto[],
  index: number,
  sort: string | undefined,
  showCommunityScore: boolean,
  t: TranslateFunction,
) {
  const s = series[index];
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

  const displayName = s.name === "unclassified" ? t("books.unclassified") : s.name;
  const allRead = s.book_count > 0 && s.books_read_count >= s.book_count;
  const statusTone = s.series_status ? seriesStatusTones[s.series_status] ?? "muted" : null;

  return { s, year, showYearGroup, showCommunityGroup, communityGroupLabel, displayName, allRead, statusTone };
}

/** Section separator used between grouped rows in the list view. */
function ListGroupHeader({ label }: { label: string }) {
  return (
    <div className="flex items-center gap-3 pt-2 first:pt-0">
      <span className="text-xs font-medium uppercase tracking-wide text-muted-foreground">{label}</span>
      <span className="h-px flex-1 bg-border/60" />
    </div>
  );
}

export function SeriesGrid({ series, sort, hasActiveUser, t, knownStatuses, showCommunityScore = false }: SeriesGridProps) {
  return (
    <JacketGrid className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 xl:grid-cols-6 gap-4">
      {series.map((s, index) => {
        const { showYearGroup, year, showCommunityGroup, communityGroupLabel, displayName, allRead, statusTone } = seriesDerived(series, index, sort, showCommunityScore, t);
        const firstBook = seriesCoverUrl(s);

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
            <CoverCard>
              <Link
                href={`/series/${s.series_id}`}
                aria-label={displayName}
                className="absolute inset-0 z-10 rounded-xl focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background"
              />

              <CoverImage
                src={firstBook}
                alt={t("books.coverOf", { name: s.name })}
                fallback="series"
                dimmed={allRead}
              />

              {(s.series_status || (s.missing_count != null && s.missing_count > 0)) && (
                <div className="pointer-events-none absolute inset-x-0 top-0 z-20 flex items-start justify-between gap-1 p-2">
                  {s.series_status && statusTone && (
                    <CoverBadge tone={statusTone}>
                      {knownStatuses[s.series_status] || s.series_status}
                    </CoverBadge>
                  )}
                  {s.missing_count != null && s.missing_count > 0 && (
                    <CoverBadge
                      tone="orange"
                      className="ml-auto"
                    >
                      <svg className="h-2.5 w-2.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2.5} aria-hidden>
                        <path d="M12 9v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" strokeLinecap="round" strokeLinejoin="round" />
                      </svg>
                      {s.missing_count}
                    </CoverBadge>
                  )}
                </div>
              )}

              <CoverProgress
                value={s.books_read_count}
                max={s.book_count}
                label={t("series.readCount", { read: String(s.books_read_count), total: String(s.book_count), plural: s.book_count !== 1 ? "s" : "" })}
              />

              <CoverScrim reserveActions>
                <h3 className="text-xs font-medium text-white line-clamp-2" title={s.name}>
                  {displayName}
                </h3>
                <div className="mt-1.5 flex items-center gap-2">
                  <span
                    className={`rounded-full px-1.5 py-0.5 text-3xs font-medium ${
                      allRead
                        ? "bg-success/90 text-success-foreground"
                        : s.books_read_count > 0
                          ? "bg-info/90 text-info-foreground"
                          : "bg-white/15 text-white/90"
                    }`}
                  >
                    {t("series.readCount", { read: String(s.books_read_count), total: String(s.book_count), plural: s.book_count !== 1 ? "s" : "" })}
                  </span>
                </div>
              </CoverScrim>

              {/* Always visible so the shortcuts stay reachable on touch. */}
              <div className="absolute bottom-2 right-2 z-30">
                <SeriesCardActions
                  libraryId={s.library_id}
                  seriesId={s.series_id}
                  seriesName={s.name}
                  bookCount={s.book_count}
                  booksReadCount={s.books_read_count}
                  hasActiveUser={hasActiveUser}
                  variant="cover"
                />
              </div>
            </CoverCard>
          </Fragment>
        );
      })}
    </JacketGrid>
  );
}

/**
 * List view of the same series data. Cover-first rows (shared `CoverRow`
 * chrome), metadata inline, reading progress and the mark-read action always
 * reachable — no hover required, so it works on touch.
 */
export function SeriesList({ series, sort, hasActiveUser, t, knownStatuses, showCommunityScore = false }: SeriesGridProps) {
  return (
    <div className="flex flex-col gap-2">
      {series.map((s, index) => {
        const { showYearGroup, year, showCommunityGroup, communityGroupLabel, displayName, allRead, statusTone } = seriesDerived(series, index, sort, showCommunityScore, t);
        const readLabel = t("series.readCount", {
          read: String(s.books_read_count),
          total: String(s.book_count),
          plural: s.book_count !== 1 ? "s" : "",
        });
        const readPercent = s.book_count > 0 ? Math.round((s.books_read_count / s.book_count) * 100) : 0;

        return (
          <Fragment key={s.series_id}>
            {showYearGroup && <ListGroupHeader label={year != null ? String(year) : t("series.noReleaseDate")} />}
            {showCommunityGroup && <ListGroupHeader label={communityGroupLabel} />}
            <CoverRow>
              <Link
                href={`/series/${s.series_id}`}
                aria-label={displayName}
                className="absolute inset-0 z-10 rounded-xl focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring"
              />

              <div className="relative aspect-[2/3] w-12 shrink-0 overflow-hidden rounded-lg bg-muted sm:w-14">
                <CoverImage
                  src={seriesCoverUrl(s)}
                  alt={t("books.coverOf", { name: s.name })}
                  fallback="series"
                  dimmed={allRead}
                  sizes="64px"
                />
              </div>

              <div className="flex min-w-0 flex-1 flex-col justify-center gap-1.5">
                <div className="flex min-w-0 items-center gap-2">
                  <h3
                    className="min-w-0 flex-1 truncate text-sm font-semibold text-foreground transition-colors group-hover:text-primary"
                    title={displayName}
                  >
                    {displayName}
                  </h3>
                  {statusTone && s.series_status && (
                    <CoverBadge tone={statusTone} className="z-20 shrink-0">
                      {knownStatuses[s.series_status] || s.series_status}
                    </CoverBadge>
                  )}
                </div>

                <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground">
                  <span>{readLabel}</span>
                  {s.start_year != null && <span className="tabular-nums">{s.start_year}</span>}
                  {s.genres.slice(0, 2).map((genre) => (
                    <span key={genre} className="rounded-full bg-muted px-1.5 py-0.5 text-2xs">
                      {genre}
                    </span>
                  ))}
                  {s.metadata_provider && (
                    <span className="rounded-full bg-muted px-1.5 py-0.5 text-2xs">
                      {providerLabel(s.metadata_provider)}
                    </span>
                  )}
                  {s.community_score != null && (
                    <span
                      className="inline-flex items-center gap-0.5"
                      title={t("series.sortCommunityScore")}
                    >
                      <Icon name="star" size="sm" className="text-warning" />
                      <span className="tabular-nums">{s.community_score.toFixed(1)}</span>
                    </span>
                  )}
                  {s.user_rating != null && (
                    <span
                      className="inline-flex items-center gap-0.5 text-primary"
                      title={t("series.myRatings")}
                    >
                      <Icon name="star" size="sm" />
                      <span className="tabular-nums">{s.user_rating.toFixed(1)}</span>
                    </span>
                  )}
                  {s.missing_count != null && s.missing_count > 0 && (
                    <CoverBadge tone="orange" className="z-20">
                      <svg className="h-2.5 w-2.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2.5} aria-hidden>
                        <path d="M12 9v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" strokeLinecap="round" strokeLinejoin="round" />
                      </svg>
                      {t("series.missingCount", { count: String(s.missing_count), plural: s.missing_count !== 1 ? "s" : "" })}
                    </CoverBadge>
                  )}
                </div>

                {s.books_read_count > 0 && !allRead && (
                  <span
                    className="flex items-center gap-2"
                    role="progressbar"
                    aria-valuenow={readPercent}
                    aria-valuemin={0}
                    aria-valuemax={100}
                    aria-label={readLabel}
                  >
                    <span className="h-1 w-full max-w-40 overflow-hidden rounded-full bg-muted">
                      <span className="block h-full rounded-full bg-primary" style={{ width: `${readPercent}%` }} />
                    </span>
                  </span>
                )}
              </div>

              <div className="relative z-20 shrink-0 self-center">
                <SeriesCardActions
                  libraryId={s.library_id}
                  seriesId={s.series_id}
                  seriesName={s.name}
                  bookCount={s.book_count}
                  booksReadCount={s.books_read_count}
                  hasActiveUser={hasActiveUser}
                  variant="inline"
                />
              </div>
            </CoverRow>
          </Fragment>
        );
      })}
    </div>
  );
}
