import React, { Suspense } from "react";
import { fetchStats } from "@/lib/api";
import { TimeSeriesCharts } from "@/app/components/TimeSeriesCharts";
import Link from "next/link";
import { getServerTranslations } from "@/lib/i18n/server";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";
import { StatsOverviewSection } from "./_components/StatsOverviewSection";
import { StatsBreakdownSection } from "./_components/StatsBreakdownSection";

export const dynamic = "force-dynamic";

// ─── Skeletons ────────────────────────────────────────────────────────────────

function OverviewSkeleton() {
  return (
    <div className="space-y-6">
      <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-4">
        {Array.from({ length: 6 }).map((_, i) => (
          <div key={i} className="h-20 bg-card border border-border/50 rounded-xl animate-pulse" />
        ))}
      </div>
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
        {Array.from({ length: 3 }).map((_, i) => (
          <div key={i} className="h-64 bg-card border border-border/50 rounded-xl animate-pulse" />
        ))}
      </div>
    </div>
  );
}

function TimeSeriesSkeleton() {
  return <div className="h-80 bg-card border border-border/50 rounded-xl animate-pulse" />;
}

function BreakdownSkeleton() {
  return (
    <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
      <div className="h-64 bg-card border border-border/50 rounded-xl animate-pulse" />
      <div className="h-64 bg-card border border-border/50 rounded-xl animate-pulse" />
    </div>
  );
}

// ─── Time-series section (server-fetches initial "week" data) ─────────────────

type TimeSeriesLabels = React.ComponentProps<typeof TimeSeriesCharts>["labels"];

async function TimeSeriesSection({ locale, labels }: { locale: string; labels: TimeSeriesLabels }) {
  const stats = await fetchStats("week");
  return (
    <TimeSeriesCharts
      initialData={{
        reading_over_time: stats.reading_over_time ?? [],
        users_reading_over_time: stats.users_reading_over_time ?? [],
        additions_over_time: stats.additions_over_time ?? [],
        jobs_over_time: stats.jobs_over_time ?? [],
      }}
      locale={locale}
      labels={labels}
    />
  );
}

// ─── Quick links (static, no fetch) ──────────────────────────────────────────

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
            <svg className={`w-5 h-5 ${l.text} ${l.hoverText}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">{l.icon}</svg>
          </div>
          <span className="font-medium text-foreground text-sm">{l.label}</span>
        </Link>
      ))}
    </div>
  );
}

// ─── Page ─────────────────────────────────────────────────────────────────────

export default async function DashboardPage() {
  const { t, locale } = await getServerTranslations();

  const timeSeriesLabels: TimeSeriesLabels = {
    readingActivity: t("dashboard.readingActivity"),
    booksAdded: t("dashboard.booksAdded"),
    jobsOverTime: t("dashboard.jobsOverTime"),
    periodDay: t("dashboard.periodDay"),
    periodWeek: t("dashboard.periodWeek"),
    periodMonth: t("dashboard.periodMonth"),
    metricBooks: t("dashboard.metricBooks"),
    metricPages: t("dashboard.metricPages"),
    noData: t("common.noData"),
    jobScan: t("dashboard.jobScan"),
    jobRebuild: t("dashboard.jobRebuild"),
    jobThumbnail: t("dashboard.jobThumbnail"),
    jobMetadata: t("dashboard.jobMetadata"),
    jobDownloads: t("dashboard.jobDownloads"),
    jobReading: t("dashboard.jobReading"),
    jobConversion: t("dashboard.jobConversion"),
  };

  return (
    <div className="max-w-7xl mx-auto space-y-6">
      {/* Header — renders immediately, no fetch */}
      <div className="mb-2">
        <h1 className="text-3xl font-bold text-foreground flex items-center gap-3">
          <svg className="w-8 h-8 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
          </svg>
          {t("dashboard.title")}
        </h1>
        <p className="text-muted-foreground mt-2 max-w-2xl">{t("dashboard.subtitle")}</p>
      </div>

      {/* Overview: stat cards + currently reading + distribution charts */}
      <Suspense fallback={<OverviewSkeleton />}>
        <StatsOverviewSection t={t} locale={locale} />
      </Suspense>

      {/* Time-series charts with period selector */}
      <Suspense fallback={<TimeSeriesSkeleton />}>
        <TimeSeriesSection locale={locale} labels={timeSeriesLabels} />
      </Suspense>

      {/* Breakdown: libraries + top series + downloads */}
      <Suspense fallback={<BreakdownSkeleton />}>
        <StatsBreakdownSection t={t} locale={locale} />
      </Suspense>

      {/* Quick links — always visible, no fetch */}
      <QuickLinks t={t} />
    </div>
  );
}
