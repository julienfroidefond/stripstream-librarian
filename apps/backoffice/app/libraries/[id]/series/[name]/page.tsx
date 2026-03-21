import { fetchLibraries, fetchBooks, fetchSeriesMetadata, getBookCoverUrl, getMetadataLink, getMissingBooks, BookDto, SeriesMetadataDto, ExternalMetadataLinkDto, MissingBooksDto } from "../../../../../lib/api";
import { BooksGrid, EmptyState } from "../../../../components/BookCard";
import { MarkSeriesReadButton } from "../../../../components/MarkSeriesReadButton";
import { MarkBookReadButton } from "../../../../components/MarkBookReadButton";
import nextDynamic from "next/dynamic";
import { OffsetPagination } from "../../../../components/ui";
import { SafeHtml } from "../../../../components/SafeHtml";
import Image from "next/image";
import Link from "next/link";

const EditSeriesForm = nextDynamic(
  () => import("../../../../components/EditSeriesForm").then(m => m.EditSeriesForm)
);
const MetadataSearchModal = nextDynamic(
  () => import("../../../../components/MetadataSearchModal").then(m => m.MetadataSearchModal)
);
const ProwlarrSearchModal = nextDynamic(
  () => import("../../../../components/ProwlarrSearchModal").then(m => m.ProwlarrSearchModal)
);
import { notFound } from "next/navigation";
import { getServerTranslations } from "../../../../../lib/i18n/server";

export const dynamic = "force-dynamic";

export default async function SeriesDetailPage({
  params,
  searchParams,
}: {
  params: Promise<{ id: string; name: string }>;
  searchParams: Promise<{ [key: string]: string | string[] | undefined }>;
}) {
  const { id, name } = await params;
  const { t } = await getServerTranslations();
  const searchParamsAwaited = await searchParams;
  const page = typeof searchParamsAwaited.page === "string" ? parseInt(searchParamsAwaited.page) : 1;
  const limit = typeof searchParamsAwaited.limit === "string" ? parseInt(searchParamsAwaited.limit) : 50;

  const seriesName = decodeURIComponent(name);

  const [library, booksPage, seriesMeta, metadataLinks] = await Promise.all([
    fetchLibraries().then((libs) => libs.find((l) => l.id === id)),
    fetchBooks(id, seriesName, page, limit).catch(() => ({
      items: [] as BookDto[],
      total: 0,
      page: 1,
      limit,
    })),
    fetchSeriesMetadata(id, seriesName).catch(() => null as SeriesMetadataDto | null),
    getMetadataLink(id, seriesName).catch(() => [] as ExternalMetadataLinkDto[]),
  ]);

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

  // Use first book cover as series cover
  const coverBookId = booksPage.items[0]?.id;

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
          href={`/libraries/${id}/series`}
          className="text-muted-foreground hover:text-primary transition-colors"
        >
          {library.name}
        </Link>
        <span className="text-muted-foreground">/</span>
        <span className="text-foreground font-medium">{displayName}</span>
      </div>

      {/* Series Header */}
      <div className="flex flex-col sm:flex-row gap-6">
        {coverBookId && (
          <div className="flex-shrink-0">
            <div className="w-40 aspect-[2/3] relative rounded-xl overflow-hidden shadow-card border border-border">
              <Image
                src={getBookCoverUrl(coverBookId)}
                alt={t("books.coverOf", { name: displayName })}
                fill
                className="object-cover"
                sizes="160px"
              />
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
              seriesName={seriesName}
              bookCount={booksPage.total}
              booksReadCount={booksReadCount}
            />
            <EditSeriesForm
              libraryId={id}
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
              missingBooks={missingData?.missing_books ?? null}
            />
            <MetadataSearchModal
              libraryId={id}
              seriesName={seriesName}
              existingLink={existingLink}
              initialMissing={missingData}
            />
          </div>
        </div>
      </div>

      {/* Books Grid */}
      {books.length > 0 ? (
        <>
          <BooksGrid books={books} />
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
