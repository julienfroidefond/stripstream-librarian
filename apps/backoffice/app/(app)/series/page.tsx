import { fetchAllSeries, fetchLibraries, fetchSeriesStatuses, LibraryDto, SeriesDto, SeriesPageDto, getBookCoverUrl } from "@/lib/api";
import { getServerTranslations } from "@/lib/i18n/server";
import { paramString, paramStringOr, paramInt, paramBool } from "@/lib/searchParams";
import { MarkSeriesReadButton } from "@/app/components/MarkSeriesReadButton";
import { LiveSearchForm } from "@/app/components/LiveSearchForm";
import { Card, CardContent, OffsetPagination } from "@/app/components/ui";
import Image from "next/image";
import Link from "next/link";
import { ProviderIcon } from "@/app/components/ProviderIcon";
import { ExternalLinkBadge } from "@/app/components/ExternalLinkBadge";

export const dynamic = "force-dynamic";

export default async function SeriesPage({
  searchParams,
}: {
  searchParams: Promise<{ [key: string]: string | string[] | undefined }>;
}) {
  const { t } = await getServerTranslations();
  const sp = await searchParams;
  const libraryId = paramString(sp, "library");
  const searchQuery = paramStringOr(sp, "q", "");
  const readingStatus = paramString(sp, "status");
  const sort = paramString(sp, "sort");
  const seriesStatus = paramString(sp, "series_status");
  const hasMissing = paramBool(sp, "has_missing");
  const metadataProvider = paramString(sp, "metadata_provider");
  const page = paramInt(sp, "page", 1);
  const limit = paramInt(sp, "limit", 20);

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
            initialValues={{
              q: searchQuery,
              library: libraryId || "",
              status: readingStatus || "",
              series_status: seriesStatus || "",
              has_missing: hasMissing ? "true" : "",
              metadata_provider: metadataProvider || "",
              sort: sort || "",
            }}
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
              <div key={s.series_id} className="group relative">
                <div
                  className={`bg-card rounded-xl shadow-sm border border-border/60 overflow-hidden group-hover:shadow-md group-hover:-translate-y-1 transition-all duration-200 ${
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
                      <div className="relative z-20">
                        <MarkSeriesReadButton
                          seriesId={s.series_id}
                          seriesName={s.name}
                          bookCount={s.book_count}
                          booksReadCount={s.books_read_count}
                        />
                      </div>
                    </div>
                    <div className="relative z-20 flex items-center gap-1 mt-1.5 flex-wrap">
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
                      {s.anilist_id && (
                        <ExternalLinkBadge
                          href={s.anilist_url ?? `https://anilist.co/manga/${s.anilist_id}`}
                          className="text-[10px] px-1.5 py-0.5 rounded-full font-medium bg-cyan-500/15 text-cyan-600 hover:bg-cyan-500/25"
                        >
                          AL
                        </ExternalLinkBadge>
                      )}
                    </div>
                  </div>
                </div>
                {/* Link overlay covering the full card — below interactive elements */}
                <Link
                  href={`/libraries/${s.library_id}/series/${s.series_id}`}
                  className="absolute inset-0 z-10 rounded-xl"
                  aria-label={s.name === "unclassified" ? t("books.unclassified") : s.name}
                />
              </div>
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
