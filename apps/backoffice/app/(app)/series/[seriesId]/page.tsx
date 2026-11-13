import { getBookCoverUrl } from "@/lib/api";
import { loadSeriesDetail } from "@/lib/series/detail";
import { cookies } from "next/headers";
import { BooksGrid, BooksList, EmptyState } from "@/app/components/BookCard";
import { SeriesRelatedCarousel } from "@/app/components/SeriesRelatedCarousel";
import { BooksGridWithMissingToggle } from "@/app/components/ShowMissingToggle";
import { ProviderIcon, providerLabel } from "@/app/components/ProviderIcon";
import { SeriesActionsToolbar } from "@/app/components/SeriesActionsToolbar";
import { SeriesRatingControl } from "@/app/components/SeriesRatingControl";
import { SeriesUserRatings, SeriesUserReading } from "@/app/components/SeriesUserActivity";
import { progressLabel } from "@/app/components/SeriesGrid";
import { OffsetPagination } from "@/app/components/ui";
import { SafeHtml } from "@/app/components/SafeHtml";
import { Icon } from "@/app/components/ui/Icon";
import Image from "next/image";
import Link from "next/link";
import { notFound } from "next/navigation";
import { getServerTranslations } from "@/lib/i18n/server";
import { parseViewMode, VIEW_MODE_COOKIE } from "@/lib/viewMode";

export const dynamic = "force-dynamic";

export default async function SeriesDetailPage({
  params,
  searchParams,
}: {
  params: Promise<{ seriesId: string }>;
  searchParams: Promise<{ [key: string]: string | string[] | undefined }>;
}) {
  const { seriesId } = await params;
  const { t } = await getServerTranslations();
  const cookieStore = await cookies();
  const viewMode = parseViewMode(cookieStore.get(VIEW_MODE_COOKIE)?.value);
  const searchParamsAwaited = await searchParams;
  const page = typeof searchParamsAwaited.page === "string" ? parseInt(searchParamsAwaited.page) : 1;
  const limit = typeof searchParamsAwaited.limit === "string" ? parseInt(searchParamsAwaited.limit) : 24;

  const result = await loadSeriesDetail(seriesId, { page, limit, viewMode });
  if (result.status === "not-found") {
    notFound();
  }

  const {
    library,
    series,
    seriesName,
    books,
    booksTotal,
    totalPages,
    booksReadCount,
    hasActiveUser,
    activeUserId,
    metadata,
    existingLink,
    missingData,
    readingStatusLink,
    relatedSeries,
    readingLists,
    ratings,
    ratedEntries,
    readingEntries,
    actions,
  } = result.model;

  const libraryId = library.id;
  const hasUserReading = readingEntries.length > 0;
  const displayName = seriesName === "unclassified" ? t("books.unclassified") : seriesName;

  // Use first_book_id from series DTO (already prioritizes regular volumes over HS),
  // fallback to provider cover_url
  const coverBookId = series.first_book_id;
  const coverBookUpdatedAt = series.first_book_updated_at;
  const seriesCoverUrl = series.cover_url;

  return (
    <div className="space-y-6">
      {/* Breadcrumb */}
      <div className="flex items-center gap-2 text-sm">
        <Link
          href="/libraries"
          className="text-muted-foreground hover:text-primary transition-colors"
        >
          {t("nav.libraries")}
        </Link>
        <span className="text-muted-foreground">/</span>
        <Link
          href={`/libraries/${libraryId}/series`}
          className="text-muted-foreground hover:text-primary transition-colors"
        >
          {library.name}
        </Link>
        <span className="text-muted-foreground">/</span>
        <span className="text-foreground font-medium">{displayName}</span>
      </div>

      {/* Series Header — the whole band lifts off the page, the cover rises
          into its frame from behind the bottom edge. */}
      <div className="flex flex-col sm:flex-row gap-6 animate-lift">
        {(coverBookId || seriesCoverUrl) && (
          <div className="flex-shrink-0">
            <div className="w-40 aspect-[2/3] relative rounded-xl overflow-hidden shadow-card border border-border">
              {coverBookId ? (
                <Image
                  src={getBookCoverUrl(coverBookId, coverBookUpdatedAt)}
                  alt={t("books.coverOf", { name: displayName })}
                  fill
                  className="object-cover animate-cover-rise"
                  sizes="160px"
                />
              ) : (
                /* eslint-disable-next-line @next/next/no-img-element */
                <img
                  src={seriesCoverUrl!}
                  alt={t("books.coverOf", { name: displayName })}
                  className="w-full h-full object-cover animate-cover-rise"
                />
              )}
            </div>
          </div>
        )}

        <div className="flex-1 space-y-4">
          <h1 className="text-3xl font-bold text-foreground">
            <span className="block overflow-hidden pb-0.5">
              <span className="block animate-mask-rise" style={{ animationDelay: "160ms" }}>
                {displayName}
              </span>
            </span>
          </h1>

          <div className="flex flex-wrap items-center gap-3">
            {metadata && metadata.authors.length > 0 && (
              <p className="text-base text-muted-foreground">{metadata.authors.join(", ")}</p>
            )}
            {metadata?.status && (
              <span className={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium ${
                metadata.status === "ongoing" ? "bg-info/15 text-info" :
                metadata.status === "ended" ? "bg-success/15 text-success" :
                metadata.status === "hiatus" ? "bg-warning/15 text-warning" :
                metadata.status === "cancelled" ? "bg-error/15 text-error" :
                "bg-muted text-muted-foreground"
              }`}>
                {t(`seriesStatus.${metadata.status}` as any) || metadata.status}
              </span>
            )}
            {existingLink?.status === "approved" && (
              existingLink.external_url ? (
                <a
                  href={existingLink.external_url}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-primary/10 text-primary text-xs border border-primary/30 hover:bg-primary/20 transition-colors"
                >
                  <ProviderIcon provider={existingLink.provider} size={12} />
                  {providerLabel(existingLink.provider)}
                </a>
              ) : (
                <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-primary/10 text-primary text-xs border border-primary/30">
                  <ProviderIcon provider={existingLink.provider} size={12} />
                  {providerLabel(existingLink.provider)}
                </span>
              )
            )}
            {readingStatusLink && (
              <a
                href={readingStatusLink.anilist_url ?? `https://anilist.co/manga/${readingStatusLink.anilist_id}`}
                target="_blank"
                rel="noopener noreferrer"
                className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-primary/10 text-primary text-xs border border-primary/30 hover:bg-primary/20 transition-colors"
              >
                <svg className="w-3 h-3" viewBox="0 0 24 24" fill="currentColor">
                  <path d="M6.361 2.943 0 21.056h4.942l1.077-3.133H11.4l1.077 3.133H17.5L11.128 2.943H6.361zm1.58 11.152 1.84-5.354 1.84 5.354H7.941zM17.358 2.943v18.113h4.284V2.943h-4.284z"/>
                </svg>
                AniList
              </a>
            )}
          </div>

          {/* Ratings section */}
          <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
            {/* My rating — a reader-only action, replaced by every reader's
                read-only rating when no user is selected (admin mode). */}
            {hasActiveUser ? (
              <SeriesRatingControl
                key={activeUserId}
                seriesId={seriesId}
                initialRating={ratings?.user_rating ?? null}
                hasAniListLink={!!readingStatusLink}
              />
            ) : (
              <SeriesUserRatings items={ratedEntries} />
            )}

            {/* Provider score badges */}
            {ratings && ratings.provider_ratings.length > 0 && (
              <>
                <span className="w-px h-5 bg-border shrink-0" />
                <div className="flex flex-wrap items-center gap-2">
                  {ratings.provider_ratings.map((pr) => (
                    <div
                      key={pr.provider}
                      className="flex items-center gap-1.5 px-2 py-1 rounded-lg bg-muted/60 border border-border/60 text-xs"
                      title={pr.rating_count ? `${pr.rating_count.toLocaleString()} votes` : undefined}
                    >
                      <ProviderIcon provider={pr.provider} size={11} />
                      <span className="font-medium tabular-nums text-foreground">
                        {pr.rating.toFixed(1)}
                      </span>
                      <span className="text-muted-foreground">/{pr.rating_scale % 1 === 0 ? pr.rating_scale.toFixed(0) : pr.rating_scale}</span>
                    </div>
                  ))}
                </div>
              </>
            )}
          </div>

          {metadata && metadata.genres.length > 0 && (
            <div className="flex flex-wrap gap-1.5">
              {metadata.genres.map((g) => (
                <span key={g} className="inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium bg-success/10 text-success border border-success/20">
                  {g}
                </span>
              ))}
            </div>
          )}

          {readingLists.length > 0 && (
            <div className="flex flex-wrap gap-1.5">
              {readingLists.map((list) => (
                <Link
                  key={list.id}
                  href={`/reading-lists/${list.id}`}
                  className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-primary/10 text-primary border border-primary/20 hover:bg-primary/20 transition-colors"
                >
                  <Icon name="bookmark" size="sm" className="!w-3 !h-3" />
                  {list.name}
                </Link>
              ))}
            </div>
          )}

          {metadata?.description && (
            <SafeHtml html={metadata.description} className="text-sm text-muted-foreground leading-relaxed" />
          )}

          <div className="flex flex-wrap items-center gap-4 text-sm">
            {metadata && metadata.publishers.length > 0 && (
              <span className="text-muted-foreground">
                <span className="font-semibold text-foreground">{metadata.publishers.join(", ")}</span>
              </span>
            )}
            {metadata?.start_year && (
              <span className="text-muted-foreground">{metadata.start_year}</span>
            )}
            {((metadata && metadata.publishers.length > 0) || metadata?.start_year) && <span className="w-px h-4 bg-border" />}
            <span className="text-muted-foreground">
              <span className="font-semibold text-foreground">{booksTotal}</span> {t("dashboard.books").toLowerCase()}
            </span>
            {hasActiveUser ? (
              <>
                <span className="w-px h-4 bg-border" />
                <span className="text-muted-foreground">
                  {progressLabel({ book_count: booksTotal, books_read_count: booksReadCount }, true, t)}
                </span>

                {/* Reading progress bar */}
                <div className="flex items-center gap-2 flex-1 min-w-[120px] max-w-[200px]">
                  <div className="flex-1 h-2 bg-muted rounded-full overflow-hidden">
                    <div
                      className="h-full bg-success rounded-full transition-all"
                      style={{ width: `${booksTotal > 0 ? (booksReadCount / booksTotal) * 100 : 0}%` }}
                    />
                  </div>
                </div>
              </>
            ) : hasUserReading ? (
              <>
                <span className="w-px h-4 bg-border" />
                <SeriesUserReading items={readingEntries} />
              </>
            ) : null}

            {/* Collection progress bar (owned / expected) */}
            {missingData && missingData.total_external > 0 && (
              <>
                <span className="w-px h-4 bg-border" />
                <span className="text-muted-foreground">
                  {missingData.total_local}/{missingData.total_external} — {t("series.missingCount", { count: missingData.missing_count, plural: missingData.missing_count !== 1 ? "s" : "" })}
                </span>
                <div className="w-[150px] h-2 bg-muted rounded-full overflow-hidden">
                  <div
                    className="h-full bg-warning rounded-full transition-all"
                    style={{ width: `${Math.round((missingData.total_local / missingData.total_external) * 100)}%` }}
                  />
                </div>
              </>
            )}
          </div>

          <SeriesActionsToolbar model={actions} />
        </div>
      </div>

      {/* Books Grid — regular volumes */}
      {(() => {
        const mainBooks = books.filter((b) => b.volume_type === "regular" || b.volume_type === "integral");
        const hsBooks = books.filter((b) => b.volume_type === "hs");
        const oneshotBooks = books.filter((b) => b.volume_type === "oneshot");
        const missingForGrid = (missingData?.missing_books ?? []).map((mb) => {
          const params = new URLSearchParams({ seriesId, linkId: existingLink?.id ?? "" });
          if (mb.volume_number != null) params.set("volume", String(mb.volume_number));
          if (mb.title) params.set("title", mb.title);
          return {
            title: mb.title,
            volume_number: mb.volume_number,
            cover_url: mb.cover_url,
            href: existingLink ? `/books/missing?${params.toString()}` : undefined,
          };
        });

        return (books.length > 0 || missingForGrid.length > 0) ? (
          <>
            {(mainBooks.length > 0 || missingForGrid.length > 0) && (
              <BooksGridWithMissingToggle
                books={mainBooks}
                missingBooks={missingForGrid}
                compact
                hasActiveUser={hasActiveUser}
                viewMode={viewMode}
              />
            )}

            {hsBooks.length > 0 && (
              <div className="mt-6">
                <h3 className="text-sm font-semibold text-muted-foreground mb-3 flex items-center gap-2">
                  <span className="h-px flex-1 bg-border" />
                  <span className="text-warning">{t("volumeType.hs")}</span>
                  <span className="h-px flex-1 bg-border" />
                </h3>
                {viewMode === "list" ? (
                  <BooksList books={hsBooks} hasActiveUser={hasActiveUser} />
                ) : (
                  <BooksGrid books={hsBooks} compact hasActiveUser={hasActiveUser} />
                )}
              </div>
            )}

            {oneshotBooks.length > 0 && (
              <div className="mt-6">
                <h3 className="text-sm font-semibold text-muted-foreground mb-3 flex items-center gap-2">
                  <span className="h-px flex-1 bg-border" />
                  <span className="text-highlight">{t("volumeType.oneshot")}</span>
                  <span className="h-px flex-1 bg-border" />
                </h3>
                {viewMode === "list" ? (
                  <BooksList books={oneshotBooks} hasActiveUser={hasActiveUser} />
                ) : (
                  <BooksGrid books={oneshotBooks} compact hasActiveUser={hasActiveUser} />
                )}
              </div>
            )}

            <OffsetPagination
              currentPage={page}
              totalPages={totalPages}
              pageSize={limit}
              totalItems={booksTotal}
            />
          </>
        ) : (
          <EmptyState message={t("librarySeries.noBooksInSeries")} />
        );
      })()}

      <SeriesRelatedCarousel items={relatedSeries} t={t} />
    </div>
  );
}
