import { fetchStatsBreakdown } from "@/lib/api";
import { Card, CardContent, CardHeader, CardTitle } from "@/app/components/ui";
import { RcStackedBar, RcHorizontalBar } from "@/app/components/DashboardCharts";
import Link from "next/link";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";
import { formatBytes, formatNumber } from "@/lib/format";
import { StatCard } from "./StatCard";

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
