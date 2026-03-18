import React from "react";
import { fetchStats, StatsResponse } from "../lib/api";
import { Card, CardContent, CardHeader, CardTitle } from "./components/ui";
import Link from "next/link";
import { getServerTranslations } from "../lib/i18n/server";
import type { TranslateFunction } from "../lib/i18n/dictionaries";

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

// Donut chart via SVG
function DonutChart({ data, colors, noDataLabel, locale = "fr" }: { data: { label: string; value: number; color: string }[]; colors?: string[]; noDataLabel?: string; locale?: string }) {
  const total = data.reduce((sum, d) => sum + d.value, 0);
  if (total === 0) return <p className="text-muted-foreground text-sm text-center py-8">{noDataLabel}</p>;

  const radius = 40;
  const circumference = 2 * Math.PI * radius;
  let offset = 0;

  return (
    <div className="flex items-center gap-6">
      <svg viewBox="0 0 100 100" className="w-32 h-32 shrink-0">
        {data.map((d, i) => {
          const pct = d.value / total;
          const dashLength = pct * circumference;
          const currentOffset = offset;
          offset += dashLength;
          return (
            <circle
              key={i}
              cx="50"
              cy="50"
              r={radius}
              fill="none"
              stroke={d.color}
              strokeWidth="16"
              strokeDasharray={`${dashLength} ${circumference - dashLength}`}
              strokeDashoffset={-currentOffset}
              transform="rotate(-90 50 50)"
              className="transition-all duration-500"
            />
          );
        })}
        <text x="50" y="50" textAnchor="middle" dominantBaseline="central" className="fill-foreground text-[10px] font-bold">
          {formatNumber(total, locale)}
        </text>
      </svg>
      <div className="flex flex-col gap-1.5 min-w-0">
        {data.map((d, i) => (
          <div key={i} className="flex items-center gap-2 text-sm">
            <span className="w-3 h-3 rounded-full shrink-0" style={{ backgroundColor: d.color }} />
            <span className="text-muted-foreground truncate">{d.label}</span>
            <span className="font-medium text-foreground ml-auto">{d.value}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

// Bar chart via pure CSS
function BarChart({ data, color = "var(--color-primary)", noDataLabel }: { data: { label: string; value: number }[]; color?: string; noDataLabel?: string }) {
  const max = Math.max(...data.map((d) => d.value), 1);
  if (data.length === 0) return <p className="text-muted-foreground text-sm text-center py-8">{noDataLabel}</p>;

  return (
    <div className="flex items-end gap-1.5 h-40">
      {data.map((d, i) => (
        <div key={i} className="flex-1 flex flex-col items-center gap-1 min-w-0">
          <span className="text-[10px] text-muted-foreground font-medium">{d.value || ""}</span>
          <div
            className="w-full rounded-t-sm transition-all duration-500 min-h-[2px]"
            style={{
              height: `${(d.value / max) * 100}%`,
              backgroundColor: color,
              opacity: d.value === 0 ? 0.2 : 1,
            }}
          />
          <span className="text-[10px] text-muted-foreground truncate w-full text-center">
            {d.label}
          </span>
        </div>
      ))}
    </div>
  );
}

// Horizontal progress bar for library breakdown
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

export default async function DashboardPage() {
  const { t, locale } = await getServerTranslations();

  let stats: StatsResponse | null = null;
  try {
    stats = await fetchStats();
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

  const { overview, reading_status, by_format, by_language, by_library, top_series, additions_over_time } = stats;

  const readingColors = ["hsl(220 13% 70%)", "hsl(45 93% 47%)", "hsl(142 60% 45%)"];
  const formatColors = [
    "hsl(198 78% 37%)", "hsl(142 60% 45%)", "hsl(45 93% 47%)",
    "hsl(2 72% 48%)", "hsl(280 60% 50%)", "hsl(32 80% 50%)",
    "hsl(170 60% 45%)", "hsl(220 60% 50%)",
  ];

  const maxLibBooks = Math.max(...by_library.map((l) => l.book_count), 1);
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

      {/* Charts row */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
        {/* Reading status donut */}
        <Card hover={false}>
          <CardHeader>
            <CardTitle className="text-base">{t("dashboard.readingStatus")}</CardTitle>
          </CardHeader>
          <CardContent>
            <DonutChart
              locale={locale}
              noDataLabel={noDataLabel}
              data={[
                { label: t("status.unread"), value: reading_status.unread, color: readingColors[0] },
                { label: t("status.reading"), value: reading_status.reading, color: readingColors[1] },
                { label: t("status.read"), value: reading_status.read, color: readingColors[2] },
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
            <DonutChart
              locale={locale}
              noDataLabel={noDataLabel}
              data={by_format.slice(0, 6).map((f, i) => ({
                label: (f.format || t("dashboard.unknown")).toUpperCase(),
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
            <DonutChart
              locale={locale}
              noDataLabel={noDataLabel}
              data={by_library.slice(0, 6).map((l, i) => ({
                label: l.library_name,
                value: l.book_count,
                color: formatColors[i % formatColors.length],
              }))}
            />
          </CardContent>
        </Card>
      </div>

      {/* Second row */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Monthly additions bar chart */}
        <Card hover={false}>
          <CardHeader>
            <CardTitle className="text-base">{t("dashboard.booksAdded")}</CardTitle>
          </CardHeader>
          <CardContent>
            <BarChart
              noDataLabel={noDataLabel}
              data={additions_over_time.map((m) => ({
                label: m.month.slice(5), // "MM" from "YYYY-MM"
                value: m.books_added,
              }))}
              color="hsl(198 78% 37%)"
            />
          </CardContent>
        </Card>

        {/* Top series */}
        <Card hover={false}>
          <CardHeader>
            <CardTitle className="text-base">{t("dashboard.popularSeries")}</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-3">
              {top_series.slice(0, 8).map((s, i) => (
                <HorizontalBar
                  key={i}
                  label={s.series}
                  value={s.book_count}
                  max={top_series[0]?.book_count || 1}
                  subLabel={t("dashboard.readCount", { read: s.read_count, total: s.book_count })}
                  color="hsl(142 60% 45%)"
                />
              ))}
              {top_series.length === 0 && (
                <p className="text-muted-foreground text-sm text-center py-4">{t("dashboard.noSeries")}</p>
              )}
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Libraries breakdown */}
      {by_library.length > 0 && (
        <Card hover={false}>
          <CardHeader>
            <CardTitle className="text-base">{t("dashboard.libraries")}</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-x-8 gap-y-4">
              {by_library.map((lib, i) => (
                <div key={i} className="space-y-2">
                  <div className="flex justify-between items-baseline">
                    <span className="font-medium text-foreground text-sm">{lib.library_name}</span>
                    <span className="text-xs text-muted-foreground">{formatBytes(lib.size_bytes)}</span>
                  </div>
                  <div className="h-3 bg-muted rounded-full overflow-hidden flex">
                    <div
                      className="h-full transition-all duration-500"
                      style={{ width: `${(lib.read_count / Math.max(lib.book_count, 1)) * 100}%`, backgroundColor: "hsl(142 60% 45%)" }}
                      title={`${t("status.read")} : ${lib.read_count}`}
                    />
                    <div
                      className="h-full transition-all duration-500"
                      style={{ width: `${(lib.reading_count / Math.max(lib.book_count, 1)) * 100}%`, backgroundColor: "hsl(45 93% 47%)" }}
                      title={`${t("status.reading")} : ${lib.reading_count}`}
                    />
                    <div
                      className="h-full transition-all duration-500"
                      style={{ width: `${(lib.unread_count / Math.max(lib.book_count, 1)) * 100}%`, backgroundColor: "hsl(220 13% 70%)" }}
                      title={`${t("status.unread")} : ${lib.unread_count}`}
                    />
                  </div>
                  <div className="flex gap-3 text-[11px] text-muted-foreground">
                    <span>{lib.book_count} {t("dashboard.books").toLowerCase()}</span>
                    <span className="text-success">{lib.read_count} {t("status.read").toLowerCase()}</span>
                    <span className="text-warning">{lib.reading_count} {t("status.reading").toLowerCase()}</span>
                  </div>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      )}

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
