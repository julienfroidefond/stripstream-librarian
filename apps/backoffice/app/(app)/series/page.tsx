import { fetchAllSeries, fetchLibraries, fetchSeriesStatuses, fetchReadingLists, fetchSeriesMemberships, LibraryDto, SeriesDto, SeriesPageDto, ReadingListDto, getBookCoverUrl } from "@/lib/api";
import { cookies } from "next/headers";
import { ReadingListCover } from "@/app/components/ReadingListCover";
import { getServerTranslations } from "@/lib/i18n/server";
import { paramString, paramStringOr, paramInt, paramBool } from "@/lib/searchParams";
import { MarkSeriesReadButton } from "@/app/components/MarkSeriesReadButton";
import { LiveSearchForm } from "@/app/components/LiveSearchForm";
import { RefreshButton } from "@/app/components/RefreshButton";
import { Card, CardContent, OffsetPagination } from "@/app/components/ui";
import Image from "next/image";
import Link from "next/link";
import { ProviderIcon } from "@/app/components/ProviderIcon";
import { ExternalLinkBadge } from "@/app/components/ExternalLinkBadge";
import nextDynamic from "next/dynamic";
const CreateSeriesButton = nextDynamic(
  () => import("@/app/components/CreateSeriesButton").then(m => m.CreateSeriesButton)
);
import { GroupByToggle } from "@/app/components/GroupByToggle";

export const dynamic = "force-dynamic";

export default async function SeriesPage({
  searchParams,
}: {
  searchParams: Promise<{ [key: string]: string | string[] | undefined }>;
}) {
  const { t } = await getServerTranslations();
  const cookieStore = await cookies();
  const hasActiveUser = !!cookieStore.get("as_user_id")?.value;
  const sp = await searchParams;
  const libraryId = paramString(sp, "library");
  const searchQuery = paramStringOr(sp, "q", "");
  const readingStatus = paramString(sp, "status");
  const sort = paramString(sp, "sort");
  const seriesStatus = paramString(sp, "series_status");
  const hasMissing = paramBool(sp, "has_missing");
  const booksFilter = paramString(sp, "books_filter"); // "wishlist" | "in_library" | ""
  const volumeTypeFilter = paramString(sp, "volume_type"); // "regular" | "oneshot" | "hs" | "integral" | ""
  const metadataProvider = paramString(sp, "metadata_provider");
  const groupBy = paramString(sp, "group_by"); // "reading_list" | ""
  const page = paramInt(sp, "page", 1);
  const limit = paramInt(sp, "limit", 24);

  const isGroupedByList = groupBy === "reading_list";

  const [libraries, seriesPage, dbStatuses, readingLists] = await Promise.all([
    fetchLibraries().catch(() => [] as LibraryDto[]),
    isGroupedByList
      ? Promise.resolve({ items: [] as SeriesDto[], total: 0, page: 1, limit } as SeriesPageDto)
      : fetchAllSeries(libraryId, searchQuery || undefined, readingStatus, page, limit, sort, seriesStatus, hasMissing, metadataProvider, undefined, booksFilter === "wishlist", booksFilter === "in_library", volumeTypeFilter || undefined).catch(
          () => ({ items: [] as SeriesDto[], total: 0, page: 1, limit }) as SeriesPageDto
        ),
    fetchSeriesStatuses().catch(() => [] as string[]),
    isGroupedByList ? fetchReadingLists().catch(() => [] as ReadingListDto[]) : Promise.resolve([] as ReadingListDto[]),
  ]);

  const series = seriesPage.items;
  const totalPages = Math.ceil(seriesPage.total / limit);
  const sortOptions = [
    { value: "", label: t("books.sortTitle") },
    { value: "latest", label: t("books.sortLatest") },
  ];

  const hasFilters = searchQuery || libraryId || readingStatus || sort || seriesStatus || hasMissing || booksFilter || volumeTypeFilter || metadataProvider;

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

  const wishlistOptions = [
    { value: "", label: t("common.all") },
    { value: "wishlist", label: t("series.wishlistOnly") },
    { value: "in_library", label: t("series.inLibrary") },
  ];

  const volumeTypeOptions = [
    { value: "", label: t("common.all") },
    { value: "regular", label: t("volumeType.regular") },
    { value: "oneshot", label: t("volumeType.oneshot") },
    { value: "hs", label: t("volumeType.hs") },
    { value: "integral", label: t("volumeType.integral") },
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
    { value: "senscritique", label: "SensCritique" },
  ];

  return (
    <>
      <div className="mb-6 flex items-center justify-between">
        <h1 className="text-3xl font-bold text-foreground flex items-center gap-3">
          <svg className="w-8 h-8 text-warning" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
          </svg>
          {t("series.title")}
        </h1>
        <div className="flex items-center gap-2">
          <GroupByToggle
            active={isGroupedByList}
            href={isGroupedByList ? "/series" : "/series?group_by=reading_list"}
          />
          <RefreshButton target="series" />
          <CreateSeriesButton libraries={libraries.map(lib => ({ id: lib.id, name: lib.name }))} />
        </div>
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
              books_filter: booksFilter || "",
              volume_type: volumeTypeFilter || "",
              metadata_provider: metadataProvider || "",
              sort: sort || "",
            }}
            fields={[
              { name: "q", type: "text", label: t("common.search"), placeholder: t("series.searchPlaceholder") },
              { name: "library", type: "select", label: t("books.library"), options: libraryOptions },
              { name: "status", type: "select", label: t("series.reading"), options: statusOptions },
              { name: "series_status", type: "select", label: t("editSeries.status"), options: seriesStatusOptions },
              { name: "has_missing", type: "select", label: t("series.missing"), options: missingOptions },
              { name: "books_filter", type: "select", label: t("series.wishlist"), options: wishlistOptions },
              { name: "volume_type", type: "select", label: t("series.volumeType"), options: volumeTypeOptions },
              { name: "metadata_provider", type: "select", label: t("series.metadata"), options: metadataOptions },
              { name: "sort", type: "select", label: t("books.sort"), options: sortOptions },
            ]}
          />
        </CardContent>
      </Card>

      {isGroupedByList ? (
        readingLists.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-20 gap-4 text-muted-foreground">
            <svg className="w-14 h-14 opacity-20" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1} d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
            </svg>
            <p className="text-sm">{t("readingLists.empty")}</p>
            <Link href="/reading-lists" className="text-sm text-primary hover:underline">{t("readingLists.title")}</Link>
          </div>
        ) : (
          <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 xl:grid-cols-6 gap-4">
            {readingLists.map((list) => {
              const covers = list.preview_covers;
              return (
                <Link
                  key={list.id}
                  href={`/reading-lists/${list.id}`}
                  className="group flex flex-col rounded-xl overflow-hidden border border-border/50 bg-card hover:border-border hover:shadow-lg transition-all duration-200"
                >
                  <ReadingListCover covers={covers} name={list.name} />
                  <div className="px-2 py-1.5">
                    <h3 className="font-medium text-foreground truncate text-xs" title={list.name}>{list.name}</h3>
                    <p className="text-[11px] text-muted-foreground mt-0.5">
                      {t("readingLists.seriesCount", { count: list.series_count, plural: list.series_count !== 1 ? "s" : "" })}
                    </p>
                  </div>
                </Link>
              );
            })}
          </div>
        )
      ) : (<>
      {/* Results count */}
      <p className="text-sm text-muted-foreground mb-4">
        {seriesPage.total} {t("series.title").toLowerCase()}
        {searchQuery && <> {t("series.matchingQuery")} &quot;{searchQuery}&quot;</>}
      </p>

      {series.length > 0 ? (
        <>
          <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 xl:grid-cols-6 gap-4">
            {series.map((s) => (
              <div key={s.series_id} className="group">
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
                              {KNOWN_STATUSES[s.series_status] || s.series_status}
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
      </>)}
    </>
  );
}
