import { fetchAllSeries, fetchLibraries, fetchSeriesStatuses, LibraryDto, SeriesDto, SeriesPageDto, getBookCoverUrl } from "../../lib/api";
import { getServerTranslations } from "../../lib/i18n/server";
import { MarkSeriesReadButton } from "../components/MarkSeriesReadButton";
import { LiveSearchForm } from "../components/LiveSearchForm";
import { Card, CardContent, OffsetPagination } from "../components/ui";
import Image from "next/image";
import Link from "next/link";
import { ProviderIcon } from "../components/ProviderIcon";

export const dynamic = "force-dynamic";

export default async function SeriesPage({
  searchParams,
}: {
  searchParams: Promise<{ [key: string]: string | string[] | undefined }>;
}) {
  const { t } = await getServerTranslations();
  const searchParamsAwaited = await searchParams;
  const libraryId = typeof searchParamsAwaited.library === "string" ? searchParamsAwaited.library : undefined;
  const searchQuery = typeof searchParamsAwaited.q === "string" ? searchParamsAwaited.q : "";
  const readingStatus = typeof searchParamsAwaited.status === "string" ? searchParamsAwaited.status : undefined;
  const sort = typeof searchParamsAwaited.sort === "string" ? searchParamsAwaited.sort : undefined;
  const seriesStatus = typeof searchParamsAwaited.series_status === "string" ? searchParamsAwaited.series_status : undefined;
  const hasMissing = searchParamsAwaited.has_missing === "true";
  const metadataProvider = typeof searchParamsAwaited.metadata_provider === "string" ? searchParamsAwaited.metadata_provider : undefined;
  const page = typeof searchParamsAwaited.page === "string" ? parseInt(searchParamsAwaited.page) : 1;
  const limit = typeof searchParamsAwaited.limit === "string" ? parseInt(searchParamsAwaited.limit) : 20;

  const [libraries, seriesPage, dbStatuses] = await Promise.all([
    fetchLibraries().catch(() => [] as LibraryDto[]),
    fetchAllSeries(libraryId, searchQuery || undefined, readingStatus, page, limit, sort, seriesStatus, hasMissing, metadataProvider).catch(
      () => ({ items: [] as SeriesDto[], total: 0, page: 1, limit }) as SeriesPageDto
    ),
    fetchSeriesStatuses().catch(() => [] as string[]),
  ]);

  const series = seriesPage.items;
  const totalPages = Math.ceil(seriesPage.total / limit);
  const sortOptions = [
    { value: "", label: t("books.sortTitle") },
    { value: "latest", label: t("books.sortLatest") },
  ];

  const hasFilters = searchQuery || libraryId || readingStatus || sort || seriesStatus || hasMissing || metadataProvider;

  const libraryOptions = [
    { value: "", label: t("books.allLibraries") },
    ...libraries.map((lib) => ({ value: lib.id, label: lib.name })),
  ];

  const statusOptions = [
    { value: "", label: t("common.all") },
    { value: "unread", label: t("status.unread") },
    { value: "reading", label: t("status.reading") },
    { value: "read", label: t("status.read") },
  ];

  const KNOWN_STATUSES: Record<string, string> = {
    ongoing: t("seriesStatus.ongoing"),
    ended: t("seriesStatus.ended"),
    hiatus: t("seriesStatus.hiatus"),
    cancelled: t("seriesStatus.cancelled"),
    upcoming: t("seriesStatus.upcoming"),
  };
  const seriesStatusOptions = [
    { value: "", label: t("seriesStatus.allStatuses") },
    ...dbStatuses.map((s) => ({ value: s, label: KNOWN_STATUSES[s] || s })),
  ];

  const missingOptions = [
    { value: "", label: t("common.all") },
    { value: "true", label: t("series.missingBooks") },
  ];

  const metadataOptions = [
    { value: "", label: t("series.metadataAll") },
    { value: "linked", label: t("series.metadataLinked") },
    { value: "unlinked", label: t("series.metadataUnlinked") },
    { value: "google_books", label: "Google Books" },
    { value: "open_library", label: "Open Library" },
    { value: "comicvine", label: "ComicVine" },
    { value: "anilist", label: "AniList" },
    { value: "bedetheque", label: "Bédéthèque" },
  ];

  return (
    <>
      <div className="mb-6">
        <h1 className="text-3xl font-bold text-foreground flex items-center gap-3">
          <svg className="w-8 h-8 text-warning" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
          </svg>
          {t("series.title")}
        </h1>
      </div>

      <Card className="mb-6">
        <CardContent className="pt-6">
          <LiveSearchForm
            basePath="/series"
            fields={[
              { name: "q", type: "text", label: t("common.search"), placeholder: t("series.searchPlaceholder") },
              { name: "library", type: "select", label: t("books.library"), options: libraryOptions },
              { name: "status", type: "select", label: t("series.reading"), options: statusOptions },
              { name: "series_status", type: "select", label: t("editSeries.status"), options: seriesStatusOptions },
              { name: "has_missing", type: "select", label: t("series.missing"), options: missingOptions },
              { name: "metadata_provider", type: "select", label: t("series.metadata"), options: metadataOptions },
              { name: "sort", type: "select", label: t("books.sort"), options: sortOptions },
            ]}
          />
        </CardContent>
      </Card>

      {/* Results count */}
      <p className="text-sm text-muted-foreground mb-4">
        {seriesPage.total} {t("series.title").toLowerCase()}
        {searchQuery && <> {t("series.matchingQuery")} &quot;{searchQuery}&quot;</>}
      </p>

      {/* Series Grid */}
      {series.length > 0 ? (
        <>
          <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-4">
            {series.map((s) => (
              <Link
                key={s.name}
                href={`/libraries/${s.library_id}/series/${encodeURIComponent(s.name)}`}
                className="group"
              >
                <div
                  className={`bg-card rounded-xl shadow-sm border border-border/60 overflow-hidden hover:shadow-md hover:-translate-y-1 transition-all duration-200 ${
                    s.books_read_count >= s.book_count ? "opacity-50" : ""
                  }`}
                >
                  <div className="aspect-[2/3] relative bg-muted/50">
                    <Image
                      src={getBookCoverUrl(s.first_book_id)}
                      alt={t("books.coverOf", { name: s.name })}
                      fill
                      className="object-cover"
                      sizes="(max-width: 640px) 50vw, (max-width: 768px) 33vw, (max-width: 1024px) 25vw, 16vw"
                    />
                  </div>
                  <div className="p-3">
                    <h3 className="font-medium text-foreground truncate text-sm" title={s.name}>
                      {s.name === "unclassified" ? t("books.unclassified") : s.name}
                    </h3>
                    <div className="flex items-center justify-between mt-1">
                      <p className="text-xs text-muted-foreground">
                        {t("series.readCount", { read: String(s.books_read_count), total: String(s.book_count), plural: s.book_count !== 1 ? "s" : "" })}
                      </p>
                      <MarkSeriesReadButton
                        seriesName={s.name}
                        bookCount={s.book_count}
                        booksReadCount={s.books_read_count}
                      />
                    </div>
                    <div className="flex items-center gap-1 mt-1.5 flex-wrap">
                      {s.series_status && (
                        <span className={`text-[10px] px-1.5 py-0.5 rounded-full font-medium ${
                          s.series_status === "ongoing" ? "bg-blue-500/15 text-blue-600" :
                          s.series_status === "ended" ? "bg-green-500/15 text-green-600" :
                          s.series_status === "hiatus" ? "bg-amber-500/15 text-amber-600" :
                          s.series_status === "cancelled" ? "bg-red-500/15 text-red-600" :
                          "bg-muted text-muted-foreground"
                        }`}>
                          {KNOWN_STATUSES[s.series_status] || s.series_status}
                        </span>
                      )}
                      {s.missing_count != null && s.missing_count > 0 && (
                        <span className="text-[10px] px-1.5 py-0.5 rounded-full font-medium bg-yellow-500/15 text-yellow-600">
                          {t("series.missingCount", { count: String(s.missing_count), plural: s.missing_count > 1 ? "s" : "" })}
                        </span>
                      )}
                      {s.metadata_provider && (
                        <span className="text-[10px] px-1.5 py-0.5 rounded-full font-medium bg-purple-500/15 text-purple-600 inline-flex items-center gap-0.5">
                          <ProviderIcon provider={s.metadata_provider} size={10} />
                        </span>
                      )}
                    </div>
                  </div>
                </div>
              </Link>
            ))}
          </div>

          <OffsetPagination
            currentPage={page}
            totalPages={totalPages}
            pageSize={limit}
            totalItems={seriesPage.total}
          />
        </>
      ) : (
        <div className="flex flex-col items-center justify-center py-16 text-center">
          <div className="w-16 h-16 mb-4 text-muted-foreground/30">
            <svg fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
            </svg>
          </div>
          <p className="text-muted-foreground text-lg">
            {hasFilters ? t("series.noResults") : t("series.noSeries")}
          </p>
        </div>
      )}
    </>
  );
}
