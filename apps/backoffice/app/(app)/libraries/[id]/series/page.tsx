import { fetchAllSeries, fetchLibraries, fetchSeriesStatuses, fetchMetadataProviders, LibraryDto, SeriesDto, SeriesPageDto } from "@/lib/api";
import type { CatalogQuery } from "@/lib/api";
import { cookies } from "next/headers";
import { OffsetPagination } from "@/app/components/ui";
import { SeriesGrid, SeriesList } from "@/app/components/SeriesGrid";
import { ViewModeToggle } from "@/app/components/ViewModeToggle";
import { EmptyState } from "@/app/components/BookCard";
import { LiveSearchForm, type LiveSearchField } from "@/app/components/LiveSearchForm";
import { hasActiveFilters, metadataOptions, missingBooksOptions, readingStatusOptions, seriesStatusLabels, seriesStatusOptions, sortOptions } from "@/lib/catalog/filters";
import { notFound } from "next/navigation";
import { LibrarySubPageHeader } from "@/app/components/LibrarySubPageHeader";
import { getServerTranslations } from "@/lib/i18n/server";
import { getActiveUser } from "@/lib/auth/activeUser";
import { paramString, paramInt, paramBool } from "@/lib/utils/searchParams";
import { parseViewMode, VIEW_MODE_COOKIE } from "@/lib/catalog/viewMode";

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
  const { hasActiveUser } = await getActiveUser();
  const cookieStore = await cookies();
  const viewMode = parseViewMode(cookieStore.get(VIEW_MODE_COOKIE)?.value);
  const sp = await searchParams;
  const page = paramInt(sp, "page", 1);
  const limit = paramInt(sp, "limit", 24);
  const catalogQuery: CatalogQuery = {
    libraryId: id,
    readingStatus: hasActiveUser ? paramString(sp, "status") : undefined,
    sort: paramString(sp, "sort"),
    seriesStatus: paramString(sp, "series_status"),
    hasMissing: paramBool(sp, "has_missing"),
    metadataProvider: paramString(sp, "metadata_provider"),
    page,
    limit,
  };

  const [library, seriesPage, dbStatuses] = await Promise.all([
    fetchLibraries().then(libs => libs.find(l => l.id === id)),
    fetchAllSeries(catalogQuery).catch(
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

  const KNOWN_STATUSES = seriesStatusLabels(t);

  const filterValues: Record<string, string> = {
    status: catalogQuery.readingStatus || "",
    series_status: catalogQuery.seriesStatus || "",
    has_missing: catalogQuery.hasMissing ? "true" : "",
    metadata_provider: catalogQuery.metadataProvider || "",
    sort: catalogQuery.sort || "",
  };
  const filterFields: LiveSearchField[] = [
    ...(hasActiveUser ? [{ name: "status", type: "select" as const, label: t("series.reading"), options: readingStatusOptions(t) }] : []),
    { name: "series_status", type: "select", label: t("editSeries.status"), options: seriesStatusOptions(t, dbStatuses) },
    { name: "has_missing", type: "select", label: t("series.missing"), options: missingBooksOptions(t) },
    { name: "metadata_provider", type: "select", label: t("series.metadata"), options: metadataOptions(t, metadataProviders) },
    { name: "sort", type: "select", label: t("books.sort"), options: sortOptions(t, "librarySeries") },
  ];
  const hasFilters = hasActiveFilters(filterValues, filterFields);

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
        initialValues={filterValues}
        fields={filterFields}
      />

      {/* Results count */}
      <div className="flex items-center justify-between gap-3">
        <p className="text-sm text-muted-foreground">
          {seriesPage.total} {t("series.title").toLowerCase()}
        </p>
        <ViewModeToggle viewMode={viewMode} />
      </div>

      {/* Series Grid */}
      {series.length > 0 ? (
        <>
          {viewMode === "list" ? (
            <SeriesList series={series} sort={catalogQuery.sort} hasActiveUser={hasActiveUser} t={t} knownStatuses={KNOWN_STATUSES} />
          ) : (
            <SeriesGrid series={series} sort={catalogQuery.sort} hasActiveUser={hasActiveUser} t={t} knownStatuses={KNOWN_STATUSES} />
          )}

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
