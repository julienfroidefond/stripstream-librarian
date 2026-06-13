import { fetchLibraries, fetchBooks, fetchSeriesMetadata, fetchSeriesById, fetchRelatedSeries, getBookCoverUrl, getMetadataLink, getMissingBooks, getReadingStatusLink, apiFetch, fetchDownloadsEnabled, fetchSeriesReadingLists, fetchSeriesRatings, BookDto, SeriesMetadataDto, ExternalMetadataLinkDto, MissingBooksDto, AnilistSeriesLinkDto, ReadingListDto, TelegramMonitorStatus, SeriesRatingsDto } from "@/lib/api";
import { cookies } from "next/headers";
import { BooksGrid, EmptyState } from "@/app/components/BookCard";
import { SeriesRelatedCarousel } from "@/app/components/SeriesRelatedCarousel";
import { BooksGridWithMissingToggle } from "@/app/components/ShowMissingToggle";
import { MarkBookReadButton } from "@/app/components/MarkBookReadButton";
import { ProviderIcon, providerLabel } from "@/app/components/ProviderIcon";
import { SeriesActionsToolbar } from "@/app/components/SeriesActionsToolbar";
import { SeriesRatingControl } from "@/app/components/SeriesRatingControl";
import { OffsetPagination } from "@/app/components/ui";
import { SafeHtml } from "@/app/components/SafeHtml";
import { Icon } from "@/app/components/ui/Icon";
import Image from "next/image";
import Link from "next/link";
import { notFound } from "next/navigation";
import { getServerTranslations } from "@/lib/i18n/server";

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
  const activeUserId = cookieStore.get("as_user_id")?.value ?? "admin";
  const hasActiveUser = activeUserId !== "admin";
  const searchParamsAwaited = await searchParams;
  const page = typeof searchParamsAwaited.page === "string" ? parseInt(searchParamsAwaited.page) : 1;
  const limit = typeof searchParamsAwaited.limit === "string" ? parseInt(searchParamsAwaited.limit) : 24;

  // Resolve library_id from the series
  const seriesDto = await fetchSeriesById(seriesId).catch(() => null);
  if (!seriesDto) {
    notFound();
  }
  const libraryId = seriesDto.library_id;

  const [library, seriesMeta, metadataLinks, readingStatusLink, prowlarrConfigured, qbConfigured, metadataProviders, renameFormat, renameFormatHs, telegramStatus] = await Promise.all([
    fetchLibraries().then((libs) => libs.find((l) => l.id === libraryId)),
    fetchSeriesMetadata(seriesId).catch(() => null as SeriesMetadataDto | null),
    getMetadataLink(seriesId).catch(() => [] as ExternalMetadataLinkDto[]),
    getReadingStatusLink(seriesId).catch(() => null as AnilistSeriesLinkDto | null),
    fetchDownloadsEnabled(),
    apiFetch<{ url?: string; username?: string }>("/settings/qbittorrent")
      .then(d => !!(d?.url?.trim() && d?.username?.trim()))
      .catch(() => false),
    apiFetch<{ comicvine?: { api_key?: string } }>("/settings/metadata_providers").catch(() => null),
    apiFetch<string>("/settings/rename_format").catch(() => null),
    apiFetch<string>("/settings/rename_format_hs").catch(() => null),
    apiFetch<TelegramMonitorStatus>("/telegram-monitor/status").catch(() => null),
  ]);
  const telegramEnabled = !!(telegramStatus?.configured && telegramStatus?.authorized);

  // Get series name from metadata for display
  const seriesName = seriesMeta?.series_name ?? "";

  // Fetch books, related series and ratings in parallel
  const [booksPage, relatedSeries, seriesReadingLists, seriesRatings] = await Promise.all([
    fetchBooks(libraryId, seriesId, page, limit).catch(() => ({
      items: [] as BookDto[],
      total: 0,
      page: 1,
      limit,
    })),
    fetchRelatedSeries(seriesId, 12).catch(() => []),
    fetchSeriesReadingLists(seriesId).catch(() => [] as ReadingListDto[]),
    fetchSeriesRatings(seriesId).catch(() => null as SeriesRatingsDto | null),
  ]);

  const hiddenProviders: string[] = [];
  if (!metadataProviders?.comicvine?.api_key) hiddenProviders.push("comicvine");

  const existingLink = metadataLinks.find((l) => l.status === "approved") ?? metadataLinks[0] ?? null;
  let missingData: MissingBooksDto | null = null;
  if (existingLink && existingLink.status === "approved") {
    missingData = await getMissingBooks(existingLink.id).catch(() => null);
  }

  if (!library) {
    notFound();
  }

  const books = booksPage.items.map((book) => ({
    ...book,
    coverUrl: getBookCoverUrl(book.id, book.updated_at),
  }));

  const totalPages = Math.ceil(booksPage.total / limit);
  const booksReadCount = booksPage.items.filter((b) => b.reading_status === "read").length;
  const ownedVolumes = booksPage.items.filter(b => b.volume != null && b.volume_type === "regular").map(b => b.volume as number);
  const displayName = seriesName === "unclassified" ? t("books.unclassified") : seriesName;

  // Use first_book_id from series DTO (already prioritizes regular volumes over HS),
  // fallback to provider cover_url
  const coverBookId = seriesDto.first_book_id;
  const coverBookUpdatedAt = seriesDto.first_book_updated_at;
  const seriesCoverUrl = seriesDto.cover_url;

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

      {/* Series Header */}
      <div className="flex flex-col sm:flex-row gap-6">
        {(coverBookId || seriesCoverUrl) && (
          <div className="flex-shrink-0">
            <div className="w-40 aspect-[2/3] relative rounded-xl overflow-hidden shadow-card border border-border">
              {coverBookId ? (
                <Image
                  src={getBookCoverUrl(coverBookId, coverBookUpdatedAt)}
                  alt={t("books.coverOf", { name: displayName })}
                  fill
                  className="object-cover"
                  sizes="160px"
                />
              ) : (
                /* eslint-disable-next-line @next/next/no-img-element */
                <img
                  src={seriesCoverUrl!}
                  alt={t("books.coverOf", { name: displayName })}
                  className="w-full h-full object-cover"
                />
              )}
            </div>
          </div>
        )}

        <div className="flex-1 space-y-4">
          <h1 className="text-3xl font-bold text-foreground">{displayName}</h1>

          <div className="flex flex-wrap items-center gap-3">
            {seriesMeta && seriesMeta.authors.length > 0 && (
              <p className="text-base text-muted-foreground">{seriesMeta.authors.join(", ")}</p>
            )}
            {seriesMeta?.status && (
              <span className={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium ${
                seriesMeta.status === "ongoing" ? "bg-blue-500/15 text-blue-600" :
                seriesMeta.status === "ended" ? "bg-green-500/15 text-green-600" :
                seriesMeta.status === "hiatus" ? "bg-amber-500/15 text-amber-600" :
                seriesMeta.status === "cancelled" ? "bg-red-500/15 text-red-600" :
                "bg-muted text-muted-foreground"
              }`}>
                {t(`seriesStatus.${seriesMeta.status}` as any) || seriesMeta.status}
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
                className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-cyan-500/10 text-cyan-600 text-xs border border-cyan-500/30 hover:bg-cyan-500/20 transition-colors"
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
            {/* My rating */}
            <SeriesRatingControl
              key={activeUserId}
              seriesId={seriesId}
              initialRating={seriesRatings?.user_rating ?? null}
              hasAniListLink={!!readingStatusLink}
            />

            {/* Provider score badges */}
            {seriesRatings && seriesRatings.provider_ratings.length > 0 && (
              <>
                <span className="w-px h-5 bg-border shrink-0" />
                <div className="flex flex-wrap items-center gap-2">
                  {seriesRatings.provider_ratings.map((pr) => (
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

          {seriesMeta && seriesMeta.genres.length > 0 && (
            <div className="flex flex-wrap gap-1.5">
              {seriesMeta.genres.map((g) => (
                <span key={g} className="inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium bg-success/10 text-success border border-success/20">
                  {g}
                </span>
              ))}
            </div>
          )}

          {seriesReadingLists.length > 0 && (
            <div className="flex flex-wrap gap-1.5">
              {seriesReadingLists.map((list) => (
                <Link
                  key={list.id}
                  href={`/reading-lists/${list.id}`}
                  className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-cyan-500/10 text-cyan-600 border border-cyan-500/20 hover:bg-cyan-500/20 transition-colors"
                >
                  <Icon name="bookmark" size="sm" className="!w-3 !h-3" />
                  {list.name}
                </Link>
              ))}
            </div>
          )}

          {seriesMeta?.description && (
            <SafeHtml html={seriesMeta.description} className="text-sm text-muted-foreground leading-relaxed" />
          )}

          <div className="flex flex-wrap items-center gap-4 text-sm">
            {seriesMeta && seriesMeta.publishers.length > 0 && (
              <span className="text-muted-foreground">
                <span className="font-semibold text-foreground">{seriesMeta.publishers.join(", ")}</span>
              </span>
            )}
            {seriesMeta?.start_year && (
              <span className="text-muted-foreground">{seriesMeta.start_year}</span>
            )}
            {((seriesMeta && seriesMeta.publishers.length > 0) || seriesMeta?.start_year) && <span className="w-px h-4 bg-border" />}
            <span className="text-muted-foreground">
              <span className="font-semibold text-foreground">{booksPage.total}</span> {t("dashboard.books").toLowerCase()}
            </span>
            <span className="w-px h-4 bg-border" />
            <span className="text-muted-foreground">
              {t("series.readCount", { read: String(booksReadCount), total: String(booksPage.total), plural: booksPage.total !== 1 ? "s" : "" })}
            </span>

            {/* Reading progress bar */}
            <div className="flex items-center gap-2 flex-1 min-w-[120px] max-w-[200px]">
              <div className="flex-1 h-2 bg-muted rounded-full overflow-hidden">
                <div
                  className="h-full bg-green-500 rounded-full transition-all"
                  style={{ width: `${booksPage.total > 0 ? (booksReadCount / booksPage.total) * 100 : 0}%` }}
                />
              </div>
            </div>

            {/* Collection progress bar (owned / expected) */}
            {missingData && missingData.total_external > 0 && (
              <>
                <span className="w-px h-4 bg-border" />
                <span className="text-muted-foreground">
                  {missingData.total_local}/{missingData.total_external} — {t("series.missingCount", { count: missingData.missing_count, plural: missingData.missing_count !== 1 ? "s" : "" })}
                </span>
                <div className="w-[150px] h-2 bg-muted rounded-full overflow-hidden">
                  <div
                    className="h-full bg-amber-500 rounded-full transition-all"
                    style={{ width: `${Math.round((missingData.total_local / missingData.total_external) * 100)}%` }}
                  />
                </div>
              </>
            )}
          </div>

          <SeriesActionsToolbar
            libraryId={libraryId}
            seriesId={seriesId}
            seriesName={seriesName}
            bookCount={booksPage.total}
            booksReadCount={booksReadCount}
            editAuthors={seriesMeta?.authors ?? []}
            editGenres={seriesMeta?.genres ?? []}
            editPublishers={seriesMeta?.publishers ?? []}
            editBookAuthor={seriesMeta?.book_author ?? booksPage.items[0]?.author ?? null}
            editBookLanguage={seriesMeta?.book_language ?? booksPage.items[0]?.language ?? null}
            editDescription={seriesMeta?.description ?? null}
            editStartYear={seriesMeta?.start_year ?? null}
            editTotalVolumes={seriesMeta?.total_volumes ?? null}
            editStatus={seriesMeta?.status ?? null}
            editLockedFields={seriesMeta?.locked_fields ?? {}}
            existingLink={existingLink}
            missingData={missingData}
            hiddenProviders={hiddenProviders}
            readingStatusProvider={library.reading_status_provider ?? null}
            readingStatusLink={readingStatusLink}
            prowlarrConfigured={prowlarrConfigured}
            qbConfigured={qbConfigured}
            telegramEnabled={telegramEnabled}
            ownedVolumes={ownedVolumes}
            renameFormat={typeof renameFormat === "string" ? renameFormat : null}
            renameFormatHs={typeof renameFormatHs === "string" ? renameFormatHs : null}
            hasActiveUser={hasActiveUser}
          />
        </div>
      </div>

      {/* Books Grid — regular volumes */}
      {(() => {
        const mainBooks = books.filter((b) => b.volume_type === "regular" || b.volume_type === "integral");
        const hsBooks = books.filter((b) => b.volume_type === "hs");
        const oneshotBooks = books.filter((b) => b.volume_type === "oneshot");
        const missingForGrid = (missingData?.missing_books ?? []).map((mb) => ({
          title: mb.title,
          volume_number: mb.volume_number,
          cover_url: mb.cover_url,
        }));

        return (books.length > 0 || missingForGrid.length > 0) ? (
          <>
            {(mainBooks.length > 0 || missingForGrid.length > 0) && (
              <BooksGridWithMissingToggle
                books={mainBooks}
                missingBooks={missingForGrid}
                compact
                hasActiveUser={hasActiveUser}
              />
            )}

            {hsBooks.length > 0 && (
              <div className="mt-6">
                <h3 className="text-sm font-semibold text-muted-foreground mb-3 flex items-center gap-2">
                  <span className="h-px flex-1 bg-border" />
                  <span className="text-orange-500">{t("volumeType.hs")}</span>
                  <span className="h-px flex-1 bg-border" />
                </h3>
                <BooksGrid books={hsBooks} compact hasActiveUser={hasActiveUser} />
              </div>
            )}

            {oneshotBooks.length > 0 && (
              <div className="mt-6">
                <h3 className="text-sm font-semibold text-muted-foreground mb-3 flex items-center gap-2">
                  <span className="h-px flex-1 bg-border" />
                  <span className="text-purple-500">{t("volumeType.oneshot")}</span>
                  <span className="h-px flex-1 bg-border" />
                </h3>
                <BooksGrid books={oneshotBooks} compact hasActiveUser={hasActiveUser} />
              </div>
            )}

            <OffsetPagination
              currentPage={page}
              totalPages={totalPages}
              pageSize={limit}
              totalItems={booksPage.total}
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
