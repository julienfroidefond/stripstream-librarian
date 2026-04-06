import { fetchLibraries, fetchBooks, fetchSeriesMetadata, fetchSeriesById, getBookCoverUrl, getMetadataLink, getMissingBooks, getReadingStatusLink, apiFetch, BookDto, SeriesMetadataDto, ExternalMetadataLinkDto, MissingBooksDto, AnilistSeriesLinkDto } from "@/lib/api";
import { BooksGrid, EmptyState } from "@/app/components/BookCard";
import { MarkSeriesReadButton } from "@/app/components/MarkSeriesReadButton";
import { MarkBookReadButton } from "@/app/components/MarkBookReadButton";
import { ProviderIcon, providerLabel } from "@/app/components/ProviderIcon";
import nextDynamic from "next/dynamic";
import { OffsetPagination } from "@/app/components/ui";
import { SafeHtml } from "@/app/components/SafeHtml";
import Image from "next/image";
import Link from "next/link";

const EditSeriesForm = nextDynamic(
  () => import("@/app/components/EditSeriesForm").then(m => m.EditSeriesForm)
);
const MetadataSearchModal = nextDynamic(
  () => import("@/app/components/MetadataSearchModal").then(m => m.MetadataSearchModal)
);
const ReadingStatusModal = nextDynamic(
  () => import("@/app/components/ReadingStatusModal").then(m => m.ReadingStatusModal)
);
const ProwlarrSearchModal = nextDynamic(
  () => import("@/app/components/ProwlarrSearchModal").then(m => m.ProwlarrSearchModal)
);
const DeleteSeriesButton = nextDynamic(
  () => import("@/app/components/DeleteSeriesButton").then(m => m.DeleteSeriesButton)
);
const MergeSeriesButton = nextDynamic(
  () => import("@/app/components/MergeSeriesButton").then(m => m.MergeSeriesButton)
);
const RenameSeriesBooksModal = nextDynamic(
  () => import("@/app/components/RenameSeriesBooksModal").then(m => m.RenameSeriesBooksModal)
);
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
  const searchParamsAwaited = await searchParams;
  const page = typeof searchParamsAwaited.page === "string" ? parseInt(searchParamsAwaited.page) : 1;
  const limit = typeof searchParamsAwaited.limit === "string" ? parseInt(searchParamsAwaited.limit) : 50;

  // Resolve library_id from the series
  const seriesDto = await fetchSeriesById(seriesId).catch(() => null);
  if (!seriesDto) {
    notFound();
  }
  const libraryId = seriesDto.library_id;

  const [library, seriesMeta, metadataLinks, readingStatusLink, prowlarrConfigured, qbConfigured, metadataProviders, renameFormat] = await Promise.all([
    fetchLibraries().then((libs) => libs.find((l) => l.id === libraryId)),
    fetchSeriesMetadata(seriesId).catch(() => null as SeriesMetadataDto | null),
    getMetadataLink(seriesId).catch(() => [] as ExternalMetadataLinkDto[]),
    getReadingStatusLink(seriesId).catch(() => null as AnilistSeriesLinkDto | null),
    apiFetch<{ api_key?: string }>("/settings/prowlarr")
      .then(d => !!(d?.api_key?.trim()))
      .catch(() => false),
    apiFetch<{ url?: string; username?: string }>("/settings/qbittorrent")
      .then(d => !!(d?.url?.trim() && d?.username?.trim()))
      .catch(() => false),
    apiFetch<{ comicvine?: { api_key?: string } }>("/settings/metadata_providers").catch(() => null),
    apiFetch<string>("/settings/rename_format").catch(() => null),
  ]);

  // Get series name from metadata for display
  const seriesName = seriesMeta?.series_name ?? "";

  // Fetch books using seriesId for the filter query
  const booksPage = await fetchBooks(libraryId, seriesId, page, limit).catch(() => ({
    items: [] as BookDto[],
    total: 0,
    page: 1,
    limit,
  }));

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
    coverUrl: getBookCoverUrl(book.id),
  }));

  const totalPages = Math.ceil(booksPage.total / limit);
  const booksReadCount = booksPage.items.filter((b) => b.reading_status === "read").length;
  const displayName = seriesName === "unclassified" ? t("books.unclassified") : seriesName;

  // Use first book cover as series cover, fallback to provider cover_url
  const coverBookId = booksPage.items[0]?.id;
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
                  src={getBookCoverUrl(coverBookId)}
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
                  {booksPage.total}/{missingData.total_external} — {t("series.missingCount", { count: missingData.missing_count, plural: missingData.missing_count !== 1 ? "s" : "" })}
                </span>
                <div className="w-[150px] h-2 bg-muted rounded-full overflow-hidden">
                  <div
                    className="h-full bg-amber-500 rounded-full transition-all"
                    style={{ width: `${Math.round((booksPage.total / missingData.total_external) * 100)}%` }}
                  />
                </div>
              </>
            )}
          </div>

          <div className="flex flex-wrap items-center gap-3">
            <MarkSeriesReadButton
              seriesId={seriesId}
              seriesName={seriesName}
              bookCount={booksPage.total}
              booksReadCount={booksReadCount}
            />
            <EditSeriesForm
              libraryId={libraryId}
              seriesId={seriesId}
              seriesName={seriesName}
              currentAuthors={seriesMeta?.authors ?? []}
              currentPublishers={seriesMeta?.publishers ?? []}
              currentBookAuthor={seriesMeta?.book_author ?? booksPage.items[0]?.author ?? null}
              currentBookLanguage={seriesMeta?.book_language ?? booksPage.items[0]?.language ?? null}
              currentDescription={seriesMeta?.description ?? null}
              currentStartYear={seriesMeta?.start_year ?? null}
              currentTotalVolumes={seriesMeta?.total_volumes ?? null}
              currentStatus={seriesMeta?.status ?? null}
              currentLockedFields={seriesMeta?.locked_fields ?? {}}
            />
            <ProwlarrSearchModal
              seriesName={seriesName}
              libraryId={libraryId}
              missingBooks={missingData?.missing_books ?? null}
              initialProwlarrConfigured={prowlarrConfigured}
              initialQbConfigured={qbConfigured}
            />
            <MetadataSearchModal
              libraryId={libraryId}
              seriesName={seriesName}
              existingLink={existingLink}
              initialMissing={missingData}
              initialHiddenProviders={hiddenProviders}
            />
            <ReadingStatusModal
              libraryId={libraryId}
              seriesId={seriesId}
              seriesName={seriesName}
              readingStatusProvider={library.reading_status_provider ?? null}
              existingLink={readingStatusLink}
            />
            <RenameSeriesBooksModal
              seriesId={seriesId}
              seriesName={seriesName}
              initialFormat={typeof renameFormat === "string" ? renameFormat : null}
            />
            <MergeSeriesButton
              seriesId={seriesId}
              seriesName={seriesName}
              libraryId={libraryId}
            />
            <DeleteSeriesButton
              seriesId={seriesId}
            />
          </div>
        </div>
      </div>

      {/* Books Grid */}
      {books.length > 0 ? (
        <>
          <BooksGrid books={books} compact />
          <OffsetPagination
            currentPage={page}
            totalPages={totalPages}
            pageSize={limit}
            totalItems={booksPage.total}
          />
        </>
      ) : (
        <EmptyState message={t("librarySeries.noBooksInSeries")} />
      )}
    </div>
  );
}
