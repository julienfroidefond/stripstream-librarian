import Link from "next/link";

import {
  BookDto,
  LibraryDto,
  SeriesDto,
  fetchAllSeries,
  fetchBooks,
  fetchGapSummary,
  fetchLibraries,
} from "@/lib/api";
import { getServerTranslations } from "@/lib/i18n/server";
import { paramInt, paramString, paramStringOr } from "@/lib/searchParams";
import { gapsForTab, isGapTab, isValidGap, totalCountKey, type GapTab } from "@/lib/metadataGaps";
import { LiveSearchForm } from "@/app/components/LiveSearchForm";
import { MetadataBooksTable, MetadataSeriesTable } from "@/app/components/MetadataTable";
import { Card, CardContent, Icon, OffsetPagination, PendingCount } from "@/app/components/ui";

export const dynamic = "force-dynamic";

function routeQuery(params: Record<string, string | undefined>): Record<string, string> {
  const query: Record<string, string> = {};
  for (const [key, value] of Object.entries(params)) {
    if (value) query[key] = value;
  }
  return query;
}

function chipClass(active: boolean): string {
  return `inline-flex items-center gap-2 rounded-full border px-3 py-1.5 text-sm transition-colors ${
    active
      ? "border-primary bg-primary/10 text-foreground"
      : "border-border/60 text-muted-foreground hover:bg-accent hover:text-accent-foreground"
  }`;
}

export default async function MetadataPage({
  searchParams,
}: {
  searchParams: Promise<{ [key: string]: string | string[] | undefined }>;
}) {
  const { t } = await getServerTranslations();
  const sp = await searchParams;

  const rawTab = paramString(sp, "tab");
  const tab: GapTab = isGapTab(rawTab) ? rawTab : "series";
  const libraryId = paramString(sp, "library");
  const searchQuery = paramStringOr(sp, "q", "");
  const page = paramInt(sp, "page", 1);
  const limit = paramInt(sp, "limit", 24);
  const rawGap = paramString(sp, "gap");
  const gap = isValidGap(tab, rawGap) ? rawGap : undefined;

  const [summary, libraries] = await Promise.all([
    fetchGapSummary(libraryId).catch(() => null),
    fetchLibraries().catch(() => [] as LibraryDto[]),
  ]);
  const summaryFailed = summary === null;

  const [series, books] = await Promise.all([
    tab === "series"
      ? fetchAllSeries(
          libraryId,
          searchQuery || undefined,
          undefined,
          page,
          limit,
          undefined,
          undefined,
          undefined,
          undefined,
          undefined,
          undefined,
          undefined,
          undefined,
          undefined,
          gap,
        ).catch(() => ({ items: [] as SeriesDto[], total: 0, page: 1, limit }))
      : Promise.resolve(null),
    tab === "books"
      ? fetchBooks(
          libraryId,
          undefined,
          page,
          limit,
          undefined,
          undefined,
          undefined,
          undefined,
          undefined,
          searchQuery || undefined,
          gap,
        ).catch(() => ({ items: [] as BookDto[], total: 0, page: 1, limit }))
      : Promise.resolve(null),
  ]);

  const items: (SeriesDto | BookDto)[] = tab === "series" ? series?.items ?? [] : books?.items ?? [];
  const pageTotal = tab === "series" ? series?.total ?? 0 : books?.total ?? 0;
  const totalPages = Math.ceil(pageTotal / limit);
  const total = summary ? summary[totalCountKey(tab)] : 0;

  const gaps = gapsForTab(tab);

  const knownStatuses: Record<string, string> = {
    ongoing: t("seriesStatus.ongoing"),
    ended: t("seriesStatus.ended"),
    hiatus: t("seriesStatus.hiatus"),
    cancelled: t("seriesStatus.cancelled"),
    upcoming: t("seriesStatus.upcoming"),
  };

  const libraryOptions = [
    { value: "", label: t("books.allLibraries") },
    ...libraries.map((lib) => ({ value: lib.id, label: lib.name })),
  ];

  const hasFilter = Boolean(gap || searchQuery || libraryId);

  return (
    <>
      <div className="mb-6">
        <h1 className="text-3xl font-bold text-foreground flex items-center gap-3">
          <Icon name="database" size="xl" className="text-amber-500" />
          {t("metadata.title")}
        </h1>
        <p className="mt-2 text-sm text-muted-foreground">{t("metadata.subtitle")}</p>
      </div>

      <div className="mb-6 flex gap-1 border-b border-border/60">
        {(["series", "books"] as const).map((value) => (
          <Link
            key={value}
            href={{ pathname: "/metadata", query: routeQuery({ tab: value, library: libraryId, q: searchQuery || undefined }) }}
            className={`-mb-px inline-flex items-center gap-2 border-b-2 px-4 py-2 text-sm font-medium transition-colors ${
              tab === value
                ? "border-primary text-foreground"
                : "border-transparent text-muted-foreground hover:text-foreground"
            }`}
          >
            <Icon name={value === "series" ? "series" : "books"} size="sm" />
            {t(value === "series" ? "metadata.tab.series" : "metadata.tab.books")}
          </Link>
        ))}
      </div>

      <Card className="mb-6">
        <CardContent className="pt-6">
          <LiveSearchForm
            basePath="/metadata"
            hiddenValues={{ tab }}
            initialValues={{ q: searchQuery, library: libraryId || "" }}
            fields={[
              { name: "q", type: "text", label: t("common.search"), placeholder: t("metadata.gapsSearchPlaceholder") },
              { name: "library", type: "select", label: t("books.library"), options: libraryOptions },
            ]}
          />
        </CardContent>
      </Card>

      {summaryFailed && (
        <Card className="mb-4 border-error/40">
          <CardContent className="flex items-center gap-2 pt-6 text-sm text-error">
            <Icon name="warning" size="sm" />
            {t("metadata.summaryError")}
          </CardContent>
        </Card>
      )}

      <div className="mb-4 flex flex-wrap gap-2">
        <Link
          href={{ pathname: "/metadata", query: routeQuery({ tab, library: libraryId, q: searchQuery || undefined }) }}
          className={chipClass(!gap)}
        >
          <Icon name="layers" size="sm" />
          {t("metadata.gap.all")}
          <PendingCount count={total} />
        </Link>
        {gaps.map((def) => (
          <Link
            key={def.value}
            href={{
              pathname: "/metadata",
              query: routeQuery({ tab, gap: def.value, library: libraryId, q: searchQuery || undefined }),
            }}
            className={chipClass(gap === def.value)}
          >
            <Icon name={def.icon} size="sm" />
            {t(def.labelKey)}
            <PendingCount count={summary ? summary[def.countKey] : 0} />
          </Link>
        ))}
      </div>

      <p className="mb-4 text-sm text-muted-foreground">
        {t("metadata.resultCount", { count: String(pageTotal), plural: pageTotal !== 1 ? "s" : "" })}
      </p>

      {items.length === 0 ? (
        <Card>
          <CardContent className="pt-6 text-sm text-muted-foreground">
            {hasFilter ? t("metadata.emptyFiltered") : t("metadata.empty")}
          </CardContent>
        </Card>
      ) : (
        <>
          <Card>
            <CardContent className="p-0">
              {tab === "series" ? (
                <MetadataSeriesTable
                  series={items as SeriesDto[]}
                  t={t}
                  knownStatuses={knownStatuses}
                />
              ) : (
                <MetadataBooksTable books={items as BookDto[]} t={t} />
              )}
            </CardContent>
          </Card>

          <OffsetPagination
            currentPage={page}
            totalPages={totalPages}
            pageSize={limit}
            totalItems={pageTotal}
          />
        </>
      )}
    </>
  );
}
