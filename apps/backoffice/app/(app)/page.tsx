import React from "react";
import { fetchStats, fetchUsers, StatsResponse, UserDto } from "@/lib/api";
import { Card, CardContent, CardHeader, CardTitle } from "@/app/components/ui";
import { RcDonutChart, RcBarChart, RcAreaChart, RcStackedBar, RcHorizontalBar, RcMultiLineChart } from "@/app/components/DashboardCharts";
import { PeriodToggle } from "@/app/components/PeriodToggle";
import { MetricToggle } from "@/app/components/MetricToggle";
import { CurrentlyReadingList, RecentlyReadList } from "@/app/components/ReadingUserFilter";
import Link from "next/link";
import { getServerTranslations } from "@/lib/i18n/server";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";

export const dynamic = "force-dynamic";

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
}

function formatNumber(n: number, locale: string): string {
  return n.toLocaleString(locale === "fr" ? "fr-FR" : "en-US");
}

function formatChartLabel(raw: string, period: "day" | "week" | "month", locale: string): string {
  const loc = locale === "fr" ? "fr-FR" : "en-US";
  if (period === "month") {
    // raw = "YYYY-MM"
    const [y, m] = raw.split("-");
    const d = new Date(Number(y), Number(m) - 1, 1);
    return d.toLocaleDateString(loc, { month: "short" });
  }
  if (period === "week") {
    // raw = "YYYY-MM-DD" (Monday of the week)
    const d = new Date(raw + "T00:00:00");
    return d.toLocaleDateString(loc, { day: "numeric", month: "short" });
  }
  // day: raw = "YYYY-MM-DD"
  const d = new Date(raw + "T00:00:00");
  return d.toLocaleDateString(loc, { weekday: "short", day: "numeric" });
}

// Horizontal progress bar for metadata quality (stays server-rendered, no recharts needed)
function HorizontalBar({ label, value, max, subLabel, color = "var(--color-primary)" }: { label: string; value: number; max: number; subLabel?: string; color?: string }) {
  const pct = max > 0 ? (value / max) * 100 : 0;
  return (
    <div className="space-y-1">
      <div className="flex justify-between text-sm">
        <span className="font-medium text-foreground truncate">{label}</span>
        <span className="text-muted-foreground shrink-0 ml-2">{subLabel || value}</span>
      </div>
      <div className="h-2 bg-muted rounded-full overflow-hidden">
        <div
          className="h-full rounded-full transition-all duration-500"
          style={{ width: `${pct}%`, backgroundColor: color }}
        />
      </div>
    </div>
  );
}

export default async function DashboardPage({
  searchParams,
}: {
  searchParams: Promise<{ [key: string]: string | string[] | undefined }>;
}) {
  const searchParamsAwaited = await searchParams;
  const rawPeriod = searchParamsAwaited.period;
  const period = rawPeriod === "day" ? "day" as const : rawPeriod === "week" ? "week" as const : "month" as const;
  const metric = searchParamsAwaited.metric === "pages" ? "pages" as const : "books" as const;
  const { t, locale } = await getServerTranslations();

  let stats: StatsResponse | null = null;
  let users: UserDto[] = [];
  try {
    [stats, users] = await Promise.all([
      fetchStats(period),
      fetchUsers().catch(() => []),
    ]);
  } catch (e) {
    console.error("Failed to fetch stats:", e);
  }

  if (!stats) {
    return (
      <div className="max-w-5xl mx-auto">
        <div className="text-center mb-12">
          <h1 className="text-4xl font-bold tracking-tight mb-4 text-foreground">StripStream Backoffice</h1>
          <p className="text-lg text-muted-foreground">{t("dashboard.loadError")}</p>
        </div>
        <QuickLinks t={t} />
      </div>
    );
  }

  const {
    overview,
    reading_status,
    currently_reading = [],
    recently_read = [],
    reading_over_time = [],
    users_reading_over_time = [],
    by_format,
    by_library,
    top_series,
    additions_over_time,
    jobs_over_time = [],
    metadata = { total_series: 0, series_linked: 0, series_unlinked: 0, books_with_summary: 0, books_with_isbn: 0, by_provider: [] },
    downloads = { active_downloads: 0, imported_downloads: 0, error_downloads: 0, total_downloads: 0, available_series: 0, total_missing_volumes: 0, recent_downloads: [] },
  } = stats;

  const readingColors = ["hsl(220 13% 70%)", "hsl(45 93% 47%)", "hsl(142 60% 45%)"];
  const formatColors = [
    "hsl(198 78% 37%)", "hsl(142 60% 45%)", "hsl(45 93% 47%)",
    "hsl(2 72% 48%)", "hsl(280 60% 50%)", "hsl(32 80% 50%)",
    "hsl(170 60% 45%)", "hsl(220 60% 50%)",
  ];

  const noDataLabel = t("common.noData");

  return (
    <div className="max-w-7xl mx-auto space-y-6">
      {/* Header */}
      <div className="mb-2">
        <h1 className="text-3xl font-bold text-foreground flex items-center gap-3">
          <svg className="w-8 h-8 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
          </svg>
          {t("dashboard.title")}
        </h1>
        <p className="text-muted-foreground mt-2 max-w-2xl">
          {t("dashboard.subtitle")}
        </p>
      </div>

      {/* Overview stat cards */}
      <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-4">
        <StatCard icon="book" label={t("dashboard.books")} value={formatNumber(overview.total_books, locale)} color="success" />
        <StatCard icon="series" label={t("dashboard.series")} value={formatNumber(overview.total_series, locale)} color="primary" />
        <StatCard icon="library" label={t("dashboard.libraries")} value={formatNumber(overview.total_libraries, locale)} color="warning" />
        <StatCard icon="pages" label={t("dashboard.pages")} value={formatNumber(overview.total_pages, locale)} color="primary" />
        <StatCard icon="author" label={t("dashboard.authors")} value={formatNumber(overview.total_authors, locale)} color="success" />
        <StatCard icon="size" label={t("dashboard.totalSize")} value={formatBytes(overview.total_size_bytes)} color="warning" />
      </div>

      {/* Currently reading + Recently read */}
      {(currently_reading.length > 0 || recently_read.length > 0) && (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          {/* Currently reading */}
          <Card hover={false}>
            <CardHeader>
              <CardTitle className="text-base">{t("dashboard.currentlyReading")}</CardTitle>
            </CardHeader>
            <CardContent>
              <CurrentlyReadingList
                items={currently_reading}
                allLabel={t("dashboard.allUsers")}
                emptyLabel={t("dashboard.noCurrentlyReading")}
                pageProgressTemplate={t("dashboard.pageProgress")}
              />
            </CardContent>
          </Card>

          {/* Recently read */}
          <Card hover={false}>
            <CardHeader>
              <CardTitle className="text-base">{t("dashboard.recentlyRead")}</CardTitle>
            </CardHeader>
            <CardContent>
              <RecentlyReadList
                items={recently_read}
                allLabel={t("dashboard.allUsers")}
                emptyLabel={t("dashboard.noRecentlyRead")}
              />
            </CardContent>
          </Card>
        </div>
      )}

      {/* Reading activity line chart */}
      <Card hover={false}>
        <CardHeader className="flex flex-row items-center justify-between space-y-0">
          <CardTitle className="text-base">{t("dashboard.readingActivity")}</CardTitle>
          <div className="flex gap-2">
            <MetricToggle labels={{ books: t("dashboard.metricBooks"), pages: t("dashboard.metricPages") }} />
            <PeriodToggle labels={{ day: t("dashboard.periodDay"), week: t("dashboard.periodWeek"), month: t("dashboard.periodMonth") }} />
          </div>
        </CardHeader>
        <CardContent>
          {(() => {
            const userColors = [
              "hsl(142 60% 45%)", "hsl(198 78% 37%)", "hsl(45 93% 47%)",
              "hsl(2 72% 48%)", "hsl(280 60% 50%)", "hsl(32 80% 50%)",
            ];
            const dataKey = metric === "pages" ? "pages_read" : "books_read";
            const usernames = [...new Set(users_reading_over_time.map(r => r.username))];
            if (usernames.length === 0) {
              return (
                <RcAreaChart
                  noDataLabel={noDataLabel}
                  data={reading_over_time.map((m) => ({ label: formatChartLabel(m.month, period, locale), value: m[dataKey] }))}
                  color="hsl(142 60% 45%)"
                />
              );
            }
            // Pivot: { label, username1: n, username2: n, ... }
            const byMonth = new Map<string, Record<string, unknown>>();
            for (const row of users_reading_over_time) {
              const label = formatChartLabel(row.month, period, locale);
              if (!byMonth.has(row.month)) byMonth.set(row.month, { label });
              byMonth.get(row.month)![row.username] = row[dataKey];
            }
            const chartData = [...byMonth.values()];
            const lines = usernames.map((u, i) => ({
              key: u,
              label: u,
              color: userColors[i % userColors.length],
            }));
            return <RcMultiLineChart data={chartData} lines={lines} noDataLabel={noDataLabel} />;
          })()}
        </CardContent>
      </Card>

      {/* Charts row */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
        {/* Reading status par lecteur */}
        <Card hover={false}>
          <CardHeader>
            <CardTitle className="text-base">{t("dashboard.readingStatus")}</CardTitle>
          </CardHeader>
          <CardContent>
            {users.length === 0 ? (
              <RcDonutChart
                noDataLabel={noDataLabel}
                data={[
                  { name: t("status.unread"), value: reading_status.unread, color: readingColors[0] },
                  { name: t("status.reading"), value: reading_status.reading, color: readingColors[1] },
                  { name: t("status.read"), value: reading_status.read, color: readingColors[2] },
                ]}
              />
            ) : (
              <div className="space-y-3">
                {users.map((user) => {
                  const total = overview.total_books;
                  const read = user.books_read;
                  const reading = user.books_reading;
                  const unread = Math.max(0, total - read - reading);
                  const readPct = total > 0 ? (read / total) * 100 : 0;
                  const readingPct = total > 0 ? (reading / total) * 100 : 0;
                  return (
                    <div key={user.id} className="space-y-1">
                      <div className="flex items-center justify-between text-sm">
                        <span className="font-medium text-foreground truncate">{user.username}</span>
                        <span className="text-xs text-muted-foreground shrink-0 ml-2">
                          <span className="text-success font-medium">{read}</span>
                          {reading > 0 && <span className="text-amber-500 font-medium"> · {reading}</span>}
                          <span className="text-muted-foreground/60"> / {total}</span>
                        </span>
                      </div>
                      <div className="h-2 bg-muted rounded-full overflow-hidden flex">
                        <div className="h-full bg-success transition-all duration-500" style={{ width: `${readPct}%` }} />
                        <div className="h-full bg-amber-500 transition-all duration-500" style={{ width: `${readingPct}%` }} />
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </CardContent>
        </Card>

        {/* By format donut */}
        <Card hover={false}>
          <CardHeader>
            <CardTitle className="text-base">{t("dashboard.byFormat")}</CardTitle>
          </CardHeader>
          <CardContent>
            <RcDonutChart
              noDataLabel={noDataLabel}
              data={by_format.slice(0, 6).map((f, i) => ({
                name: (f.format || t("dashboard.unknown")).toUpperCase(),
                value: f.count,
                color: formatColors[i % formatColors.length],
              }))}
            />
          </CardContent>
        </Card>

        {/* By library donut */}
        <Card hover={false}>
          <CardHeader>
            <CardTitle className="text-base">{t("dashboard.byLibrary")}</CardTitle>
          </CardHeader>
          <CardContent>
            <RcDonutChart
              noDataLabel={noDataLabel}
              data={by_library.slice(0, 6).map((l, i) => ({
                name: l.library_name,
                value: l.book_count,
                color: formatColors[i % formatColors.length],
              }))}
            />
          </CardContent>
        </Card>
      </div>

      {/* Metadata row */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
        {/* Series metadata coverage donut */}
        <Card hover={false}>
          <CardHeader>
            <CardTitle className="text-base">{t("dashboard.metadataCoverage")}</CardTitle>
          </CardHeader>
          <CardContent>
            <RcDonutChart
              noDataLabel={noDataLabel}
              data={[
                { name: t("dashboard.seriesLinked"), value: metadata.series_linked, color: "hsl(142 60% 45%)" },
                { name: t("dashboard.seriesUnlinked"), value: metadata.series_unlinked, color: "hsl(220 13% 70%)" },
              ]}
            />
          </CardContent>
        </Card>

        {/* By provider donut */}
        <Card hover={false}>
          <CardHeader>
            <CardTitle className="text-base">{t("dashboard.byProvider")}</CardTitle>
          </CardHeader>
          <CardContent>
            <RcDonutChart
              noDataLabel={noDataLabel}
              data={metadata.by_provider.map((p, i) => ({
                name: p.provider.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase()),
                value: p.count,
                color: formatColors[i % formatColors.length],
              }))}
            />
          </CardContent>
        </Card>

        {/* Book metadata quality */}
        <Card hover={false}>
          <CardHeader>
            <CardTitle className="text-base">{t("dashboard.bookMetadata")}</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-4">
              <HorizontalBar
                label={t("dashboard.withSummary")}
                value={metadata.books_with_summary}
                max={overview.total_books}
                subLabel={overview.total_books > 0 ? `${Math.round((metadata.books_with_summary / overview.total_books) * 100)}%` : "0%"}
                color="hsl(198 78% 37%)"
              />
              <HorizontalBar
                label={t("dashboard.withIsbn")}
                value={metadata.books_with_isbn}
                max={overview.total_books}
                subLabel={overview.total_books > 0 ? `${Math.round((metadata.books_with_isbn / overview.total_books) * 100)}%` : "0%"}
                color="hsl(280 60% 50%)"
              />
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Libraries breakdown + Top series */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {by_library.length > 0 && (
          <Card hover={false}>
            <CardHeader>
              <CardTitle className="text-base">{t("dashboard.libraries")}</CardTitle>
            </CardHeader>
            <CardContent>
              <RcStackedBar
                data={by_library.map((lib) => ({
                  name: lib.library_name,
                  read: lib.read_count,
                  reading: lib.reading_count,
                  unread: lib.unread_count,
                  sizeLabel: formatBytes(lib.size_bytes),
                }))}
                labels={{
                  read: t("status.read"),
                  reading: t("status.reading"),
                  unread: t("status.unread"),
                  books: t("dashboard.books"),
                }}
              />
            </CardContent>
          </Card>
        )}

        {/* Top series */}
        <Card hover={false}>
          <CardHeader>
            <CardTitle className="text-base">{t("dashboard.popularSeries")}</CardTitle>
          </CardHeader>
          <CardContent>
            <RcHorizontalBar
              noDataLabel={t("dashboard.noSeries")}
              data={top_series.slice(0, 8).map((s) => ({
                name: s.series,
                value: s.book_count,
                subLabel: t("dashboard.readCount", { read: s.read_count, total: s.book_count }),
              }))}
              color="hsl(142 60% 45%)"
            />
          </CardContent>
        </Card>
      </div>

      {/* Downloads section */}
      {(downloads.total_downloads > 0 || downloads.available_series > 0) && (
        <>
          <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-4">
            <StatCard icon="download" label={t("dashboard.totalDownloads")} value={formatNumber(downloads.total_downloads, locale)} color="primary" />
            <StatCard icon="active" label={t("dashboard.activeDownloads")} value={formatNumber(downloads.active_downloads, locale)} color="warning" />
            <StatCard icon="imported" label={t("dashboard.importedDownloads")} value={formatNumber(downloads.imported_downloads, locale)} color="success" />
            <StatCard icon="error" label={t("dashboard.errorDownloads")} value={formatNumber(downloads.error_downloads, locale)} color="warning" />
            <StatCard icon="available" label={t("dashboard.availableSeries")} value={formatNumber(downloads.available_series, locale)} color="primary" />
            <StatCard icon="missing" label={t("dashboard.missingVolumes")} value={formatNumber(downloads.total_missing_volumes, locale)} color="warning" />
          </div>

          {downloads.recent_downloads.length > 0 && (
            <Card hover={false}>
              <CardHeader>
                <CardTitle className="text-base flex items-center gap-2">
                  <svg className="w-4 h-4 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
                  </svg>
                  {t("dashboard.recentDownloads")}
                </CardTitle>
              </CardHeader>
              <CardContent>
                <div className="space-y-3">
                  {downloads.recent_downloads.map((dl) => {
                    const statusColors: Record<string, string> = {
                      downloading: "bg-blue-500/10 text-blue-600",
                      completed: "bg-amber-500/10 text-amber-600",
                      importing: "bg-purple-500/10 text-purple-600",
                      imported: "bg-success/10 text-success",
                      error: "bg-destructive/10 text-destructive",
                    };
                    const statusMap: Record<string, string> = {
                      downloading: t("dashboard.downloadStatus.downloading"),
                      completed: t("dashboard.downloadStatus.completed"),
                      importing: t("dashboard.downloadStatus.importing"),
                      imported: t("dashboard.downloadStatus.imported"),
                      error: t("dashboard.downloadStatus.error"),
                    };
                    const statusLabel = statusMap[dl.status] || dl.status;
                    return (
                      <div key={dl.id} className="flex items-center justify-between gap-3">
                        <div className="min-w-0 flex-1">
                          <Link href={`/downloads` as any} className="font-medium text-sm text-foreground hover:text-primary transition-colors truncate block">
                            {dl.series_name}
                          </Link>
                          <p className="text-xs text-muted-foreground">
                            {dl.expected_volumes.length > 0 && (
                              <span>{t("dashboard.volumes")} {dl.expected_volumes.join(", ")}</span>
                            )}
                            {dl.expected_volumes.length > 0 && dl.created_at && <span> · </span>}
                            {dl.created_at && <span>{dl.created_at}</span>}
                          </p>
                        </div>
                        <span className={`text-xs px-2 py-0.5 rounded-full font-medium shrink-0 ${statusColors[dl.status] || "bg-muted text-muted-foreground"}`}>
                          {statusLabel}
                        </span>
                      </div>
                    );
                  })}
                </div>
              </CardContent>
            </Card>
          )}
        </>
      )}

      {/* Additions line chart – full width */}
      <Card hover={false}>
        <CardHeader className="flex flex-row items-center justify-between space-y-0">
          <CardTitle className="text-base">{t("dashboard.booksAdded")}</CardTitle>
          <PeriodToggle labels={{ day: t("dashboard.periodDay"), week: t("dashboard.periodWeek"), month: t("dashboard.periodMonth") }} />
        </CardHeader>
        <CardContent>
          <RcAreaChart
            noDataLabel={noDataLabel}
            data={additions_over_time.map((m) => ({ label: formatChartLabel(m.month, period, locale), value: m.books_added }))}
            color="hsl(198 78% 37%)"
          />
        </CardContent>
      </Card>

      {/* Jobs over time – multi-line chart */}
      <Card hover={false}>
        <CardHeader className="flex flex-row items-center justify-between space-y-0">
          <CardTitle className="text-base">{t("dashboard.jobsOverTime")}</CardTitle>
          <PeriodToggle labels={{ day: t("dashboard.periodDay"), week: t("dashboard.periodWeek"), month: t("dashboard.periodMonth") }} />
        </CardHeader>
        <CardContent>
          <RcMultiLineChart
            noDataLabel={noDataLabel}
            data={jobs_over_time.map((j) => ({
              label: formatChartLabel(j.label, period, locale),
              scan: j.scan,
              rebuild: j.rebuild,
              thumbnail: j.thumbnail,
              other: j.other,
            }))}
            lines={[
              { key: "scan", label: t("dashboard.jobScan"), color: "hsl(198 78% 37%)" },
              { key: "rebuild", label: t("dashboard.jobRebuild"), color: "hsl(142 60% 45%)" },
              { key: "thumbnail", label: t("dashboard.jobThumbnail"), color: "hsl(45 93% 47%)" },
              { key: "other", label: t("dashboard.jobOther"), color: "hsl(280 60% 50%)" },
            ]}
          />
        </CardContent>
      </Card>

      {/* Quick links */}
      <QuickLinks t={t} />
    </div>
  );
}

function StatCard({ icon, label, value, color }: { icon: string; label: string; value: string; color: string }) {
  const icons: Record<string, React.ReactNode> = {
    book: <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />,
    series: <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />,
    library: <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />,
    pages: <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />,
    author: <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />,
    size: <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4m0 5c0 2.21-3.582 4-8 4s-8-1.79-8-4" />,
    download: <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />,
    active: <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" />,
    imported: <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />,
    error: <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4.5c-.77-.833-2.694-.833-3.464 0L3.34 16.5c-.77.833.192 2.5 1.732 2.5z" />,
    available: <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />,
    missing: <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />,
  };

  const colorClasses: Record<string, string> = {
    primary: "bg-primary/10 text-primary",
    success: "bg-success/10 text-success",
    warning: "bg-warning/10 text-warning",
  };

  return (
    <Card hover={false} className="p-4">
      <div className="flex items-center gap-3">
        <div className={`w-10 h-10 rounded-lg flex items-center justify-center shrink-0 ${colorClasses[color]}`}>
          <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            {icons[icon]}
          </svg>
        </div>
        <div className="min-w-0">
          <p className="text-xl font-bold text-foreground leading-tight">{value}</p>
          <p className="text-xs text-muted-foreground">{label}</p>
        </div>
      </div>
    </Card>
  );
}

function QuickLinks({ t }: { t: TranslateFunction }) {
  const links = [
    { href: "/libraries", label: t("nav.libraries"), bg: "bg-primary/10", text: "text-primary", hoverBg: "group-hover:bg-primary", hoverText: "group-hover:text-primary-foreground", icon: <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" /> },
    { href: "/books", label: t("nav.books"), bg: "bg-success/10", text: "text-success", hoverBg: "group-hover:bg-success", hoverText: "group-hover:text-white", icon: <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" /> },
    { href: "/series", label: t("nav.series"), bg: "bg-warning/10", text: "text-warning", hoverBg: "group-hover:bg-warning", hoverText: "group-hover:text-white", icon: <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" /> },
    { href: "/jobs", label: t("nav.jobs"), bg: "bg-destructive/10", text: "text-destructive", hoverBg: "group-hover:bg-destructive", hoverText: "group-hover:text-destructive-foreground", icon: <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" /> },
  ];

  return (
    <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
      {links.map((l) => (
        <Link
          key={l.href}
          href={l.href as any}
          className="group p-4 bg-card/80 backdrop-blur-sm rounded-xl border border-border/50 shadow-sm hover:shadow-md hover:-translate-y-0.5 transition-all duration-200 flex items-center gap-3"
        >
          <div className={`w-9 h-9 rounded-lg flex items-center justify-center transition-colors duration-200 ${l.bg} ${l.hoverBg}`}>
            <svg className={`w-5 h-5 ${l.text} ${l.hoverText}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
              {l.icon}
            </svg>
          </div>
          <span className="font-medium text-foreground text-sm">{l.label}</span>
        </Link>
      ))}
    </div>
  );
}
