import React from "react";
import { fetchStats, StatsResponse, getBookCoverUrl } from "@/lib/api";
import { Card, CardContent, CardHeader, CardTitle } from "@/app/components/ui";
import { RcDonutChart, RcBarChart, RcAreaChart, RcStackedBar, RcHorizontalBar, RcMultiLineChart } from "@/app/components/DashboardCharts";
import { PeriodToggle } from "@/app/components/PeriodToggle";
import Image from "next/image";
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
  const { t, locale } = await getServerTranslations();

  let stats: StatsResponse | null = null;
  try {
    stats = await fetchStats(period);
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
    by_format,
    by_library,
    top_series,
    additions_over_time,
    jobs_over_time = [],
    metadata = { total_series: 0, series_linked: 0, series_unlinked: 0, books_with_summary: 0, books_with_isbn: 0, by_provider: [] },
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
              {currently_reading.length === 0 ? (
                <p className="text-muted-foreground text-sm text-center py-4">{t("dashboard.noCurrentlyReading")}</p>
              ) : (
                <div className="space-y-3 max-h-[216px] overflow-y-auto pr-1">
                  {currently_reading.slice(0, 8).map((book) => {
                    const pct = book.page_count > 0 ? Math.round((book.current_page / book.page_count) * 100) : 0;
                    return (
                      <Link key={book.book_id} href={`/books/${book.book_id}` as any} className="flex items-center gap-3 group">
                        <Image
                          src={getBookCoverUrl(book.book_id)}
                          alt={book.title}
                          width={40}
                          height={56}
                          className="w-10 h-14 object-cover rounded shadow-sm shrink-0 bg-muted"
                        />
                        <div className="min-w-0 flex-1">
                          <p className="text-sm font-medium text-foreground truncate group-hover:text-primary transition-colors">{book.title}</p>
                          {book.series && <p className="text-xs text-muted-foreground truncate">{book.series}</p>}
                          <div className="mt-1.5 flex items-center gap-2">
                            <div className="h-1.5 flex-1 bg-muted rounded-full overflow-hidden">
                              <div className="h-full bg-warning rounded-full transition-all" style={{ width: `${pct}%` }} />
                            </div>
                            <span className="text-[10px] text-muted-foreground shrink-0">{pct}%</span>
                          </div>
                          <p className="text-[10px] text-muted-foreground mt-0.5">{t("dashboard.pageProgress", { current: book.current_page, total: book.page_count })}</p>
                        </div>
                      </Link>
                    );
                  })}
                </div>
              )}
            </CardContent>
          </Card>

          {/* Recently read */}
          <Card hover={false}>
            <CardHeader>
              <CardTitle className="text-base">{t("dashboard.recentlyRead")}</CardTitle>
            </CardHeader>
            <CardContent>
              {recently_read.length === 0 ? (
                <p className="text-muted-foreground text-sm text-center py-4">{t("dashboard.noRecentlyRead")}</p>
              ) : (
                <div className="space-y-3 max-h-[216px] overflow-y-auto pr-1">
                  {recently_read.map((book) => (
                    <Link key={book.book_id} href={`/books/${book.book_id}` as any} className="flex items-center gap-3 group">
                      <Image
                        src={getBookCoverUrl(book.book_id)}
                        alt={book.title}
                        width={40}
                        height={56}
                        className="w-10 h-14 object-cover rounded shadow-sm shrink-0 bg-muted"
                      />
                      <div className="min-w-0 flex-1">
                        <p className="text-sm font-medium text-foreground truncate group-hover:text-primary transition-colors">{book.title}</p>
                        {book.series && <p className="text-xs text-muted-foreground truncate">{book.series}</p>}
                      </div>
                      <span className="text-xs text-muted-foreground shrink-0">{book.last_read_at}</span>
                    </Link>
                  ))}
                </div>
              )}
            </CardContent>
          </Card>
        </div>
      )}

      {/* Reading activity line chart */}
      <Card hover={false}>
        <CardHeader className="flex flex-row items-center justify-between space-y-0">
          <CardTitle className="text-base">{t("dashboard.readingActivity")}</CardTitle>
          <PeriodToggle labels={{ day: t("dashboard.periodDay"), week: t("dashboard.periodWeek"), month: t("dashboard.periodMonth") }} />
        </CardHeader>
        <CardContent>
          <RcAreaChart
            noDataLabel={noDataLabel}
            data={reading_over_time.map((m) => ({ label: formatChartLabel(m.month, period, locale), value: m.books_read }))}
            color="hsl(142 60% 45%)"
          />
        </CardContent>
      </Card>

      {/* Charts row */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
        {/* Reading status donut */}
        <Card hover={false}>
          <CardHeader>
            <CardTitle className="text-base">{t("dashboard.readingStatus")}</CardTitle>
          </CardHeader>
          <CardContent>
            <RcDonutChart
              noDataLabel={noDataLabel}
              data={[
                { name: t("status.unread"), value: reading_status.unread, color: readingColors[0] },
                { name: t("status.reading"), value: reading_status.reading, color: readingColors[1] },
                { name: t("status.read"), value: reading_status.read, color: readingColors[2] },
              ]}
            />
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
