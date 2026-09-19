import { fetchAllSeries, fetchLibraries, fetchSeriesStatuses, fetchMetadataProviders, LibraryDto, SeriesDto, SeriesPageDto } from "@/lib/api";
import { cookies } from "next/headers";
import { OffsetPagination } from "@/app/components/ui";
import { SeriesGrid } from "@/app/components/SeriesGrid";
import { EmptyState } from "@/app/components/BookCard";
import { LiveSearchForm } from "@/app/components/LiveSearchForm";
import { notFound } from "next/navigation";
import { LibrarySubPageHeader } from "@/app/components/LibrarySubPageHeader";
import { getServerTranslations } from "@/lib/i18n/server";
import { paramString, paramInt, paramBool } from "@/lib/searchParams";

export const dynamic = "force-dynamic";

export default async function LibrarySeriesPage({
  params,
  searchParams
}: {
  params: Promise<{ id: string }>;
  searchParams: Promise<{ [key: string]: string | string[] | undefined }>;
}) {
  const { id } = await params;
  const { t } = await getServerTranslations();
  const cookieStore = await cookies();
  const hasActiveUser = !!cookieStore.get("as_user_id")?.value;
  const sp = await searchParams;
  const readingStatus = paramString(sp, "status");
  const sort = paramString(sp, "sort");
  const seriesStatus = paramString(sp, "series_status");
  const hasMissing = paramBool(sp, "has_missing");
  const metadataProvider = paramString(sp, "metadata_provider");
  const page = paramInt(sp, "page", 1);
  const limit = paramInt(sp, "limit", 24);

  const [library, seriesPage, dbStatuses] = await Promise.all([
    fetchLibraries().then(libs => libs.find(l => l.id === id)),
    fetchAllSeries(id, undefined, readingStatus, page, limit, sort, seriesStatus, hasMissing, metadataProvider).catch(
      () => ({ items: [] as SeriesDto[], total: 0, page: 1, limit }) as SeriesPageDto
    ),
    fetchSeriesStatuses().catch(() => [] as string[]),
  ]);
  const metadataProviders = await fetchMetadataProviders().catch(() => []);

  if (!library) {
    notFound();
  }

  const series = seriesPage.items;
  const totalPages = Math.ceil(seriesPage.total / limit);

  const KNOWN_STATUSES: Record<string, string> = {
    ongoing: t("seriesStatus.ongoing"),
    ended: t("seriesStatus.ended"),
    hiatus: t("seriesStatus.hiatus"),
    cancelled: t("seriesStatus.cancelled"),
    upcoming: t("seriesStatus.upcoming"),
  };

  const sortOptions = [
    { value: "", label: t("books.sortTitle") },
    { value: "latest", label: t("books.sortLatest") },
    { value: "release_date", label: t("series.sortReleaseDate") },
  ];

  const statusOptions = [
    { value: "", label: t("common.all") },
    { value: "unread", label: t("status.unread") },
    { value: "reading", label: t("status.reading") },
    { value: "read", label: t("status.read") },
  ];

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
    ...metadataProviders.map((provider) => ({ value: provider.id, label: provider.label })),
  ];

  const hasFilters = readingStatus || sort || seriesStatus || hasMissing || metadataProvider;

  return (
    <div className="space-y-6">
      <LibrarySubPageHeader
        library={library}
        title={t("series.title")}
        icon={
          <svg className="w-8 h-8" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
          </svg>
        }
        iconColor="text-primary"
      />

      <LiveSearchForm
        basePath={`/libraries/${id}/series`}
        initialValues={{
          status: readingStatus || "",
          series_status: seriesStatus || "",
          has_missing: hasMissing ? "true" : "",
          metadata_provider: metadataProvider || "",
          sort: sort || "",
        }}
        fields={[
          { name: "status", type: "select", label: t("series.reading"), options: statusOptions },
          { name: "series_status", type: "select", label: t("editSeries.status"), options: seriesStatusOptions },
          { name: "has_missing", type: "select", label: t("series.missing"), options: missingOptions },
          { name: "metadata_provider", type: "select", label: t("series.metadata"), options: metadataOptions },
          { name: "sort", type: "select", label: t("books.sort"), options: sortOptions },
        ]}
      />

      {/* Results count */}
      <p className="text-sm text-muted-foreground">
        {seriesPage.total} {t("series.title").toLowerCase()}
      </p>

      {/* Series Grid */}
      {series.length > 0 ? (
        <>
          <SeriesGrid series={series} sort={sort} hasActiveUser={hasActiveUser} t={t} knownStatuses={KNOWN_STATUSES} />

          <OffsetPagination
            currentPage={page}
            totalPages={totalPages}
            pageSize={limit}
            totalItems={seriesPage.total}
          />
        </>
      ) : (
        <EmptyState
          message={hasFilters ? t("series.noResults") : t("series.noSeries")}
          icon={<path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />}
        />
      )}
    </div>
  );
}
