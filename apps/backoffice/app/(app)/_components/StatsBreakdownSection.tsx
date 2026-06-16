import React from "react";
import { fetchStatsBreakdown } from "@/lib/api";
import { Card, CardContent, CardHeader, CardTitle } from "@/app/components/ui";
import { RcStackedBar, RcHorizontalBar } from "@/app/components/DashboardCharts";
import Link from "next/link";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";

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

function StatCard({ icon, label, value, color }: { icon: string; label: string; value: string; color: string }) {
  const icons: Record<string, React.ReactNode> = {
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
          <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">{icons[icon]}</svg>
        </div>
        <div className="min-w-0">
          <p className="text-xl font-bold text-foreground leading-tight">{value}</p>
          <p className="text-xs text-muted-foreground">{label}</p>
        </div>
      </div>
    </Card>
  );
}

type Props = {
  t: TranslateFunction;
  locale: string;
};

export async function StatsBreakdownSection({ t, locale }: Props) {
  const { by_library, top_series, downloads } = await fetchStatsBreakdown();

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

  return (
    <>
      {/* Libraries + Top series */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {by_library.length > 0 && (
          <Card hover={false}>
            <CardHeader><CardTitle className="text-base">{t("dashboard.libraries")}</CardTitle></CardHeader>
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

        <Card hover={false}>
          <CardHeader><CardTitle className="text-base">{t("dashboard.popularSeries")}</CardTitle></CardHeader>
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

      {/* Downloads */}
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
                  {downloads.recent_downloads.map((dl) => (
                    <div key={dl.id} className="flex items-center justify-between gap-3">
                      <div className="min-w-0 flex-1">
                        <Link href={`/downloads` as any} className="font-medium text-sm text-foreground hover:text-primary transition-colors truncate block">
                          {dl.series_name}
                        </Link>
                        <p className="text-xs text-muted-foreground">
                          {dl.expected_volumes.length > 0 && <span>{t("dashboard.volumes")} {dl.expected_volumes.join(", ")}</span>}
                          {dl.expected_volumes.length > 0 && dl.created_at && <span> · </span>}
                          {dl.created_at && <span>{dl.created_at}</span>}
                        </p>
                      </div>
                      <span className={`text-xs px-2 py-0.5 rounded-full font-medium shrink-0 ${statusColors[dl.status] || "bg-muted text-muted-foreground"}`}>
                        {statusMap[dl.status] || dl.status}
                      </span>
                    </div>
                  ))}
                </div>
              </CardContent>
            </Card>
          )}
        </>
      )}
    </>
  );
}
