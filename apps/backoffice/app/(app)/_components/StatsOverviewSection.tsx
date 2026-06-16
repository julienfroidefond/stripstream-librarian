import React from "react";
import { fetchStatsOverview, fetchUsers, UserDto } from "@/lib/api";
import { Card, CardContent, CardHeader, CardTitle } from "@/app/components/ui";
import { RcDonutChart } from "@/app/components/DashboardCharts";
import { CurrentlyReadingList, RecentlyReadList } from "@/app/components/ReadingUserFilter";
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

function HorizontalBar({ label, value, max, subLabel, color = "var(--color-primary)" }: { label: string; value: number; max: number; subLabel?: string; color?: string }) {
  const pct = max > 0 ? (value / max) * 100 : 0;
  return (
    <div className="space-y-1">
      <div className="flex justify-between text-sm">
        <span className="font-medium text-foreground truncate">{label}</span>
        <span className="text-muted-foreground shrink-0 ml-2">{subLabel || value}</span>
      </div>
      <div className="h-2 bg-muted rounded-full overflow-hidden">
        <div className="h-full rounded-full transition-all duration-500" style={{ width: `${pct}%`, backgroundColor: color }} />
      </div>
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

export async function StatsOverviewSection({ t, locale }: Props) {
  const [data, users] = await Promise.all([
    fetchStatsOverview(),
    fetchUsers().catch((): UserDto[] => []),
  ]);

  const { overview, reading_status, by_format, by_language, metadata, currently_reading, recently_read } = data;

  const readingColors = ["hsl(220 13% 70%)", "hsl(45 93% 47%)", "hsl(142 60% 45%)"];
  const formatColors = [
    "hsl(198 78% 37%)", "hsl(142 60% 45%)", "hsl(45 93% 47%)",
    "hsl(2 72% 48%)", "hsl(280 60% 50%)", "hsl(32 80% 50%)",
    "hsl(170 60% 45%)", "hsl(220 60% 50%)",
  ];
  const noDataLabel = t("common.noData");

  return (
    <>
      {/* Stat cards */}
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
          <Card hover={false}>
            <CardHeader><CardTitle className="text-base">{t("dashboard.currentlyReading")}</CardTitle></CardHeader>
            <CardContent>
              <CurrentlyReadingList
                items={currently_reading}
                allLabel={t("dashboard.allUsers")}
                emptyLabel={t("dashboard.noCurrentlyReading")}
                pageProgressTemplate={t("dashboard.pageProgress")}
              />
            </CardContent>
          </Card>
          <Card hover={false}>
            <CardHeader><CardTitle className="text-base">{t("dashboard.recentlyRead")}</CardTitle></CardHeader>
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

      {/* Distribution charts */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
        <Card hover={false}>
          <CardHeader><CardTitle className="text-base">{t("dashboard.readingStatus")}</CardTitle></CardHeader>
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

        <Card hover={false}>
          <CardHeader><CardTitle className="text-base">{t("dashboard.byFormat")}</CardTitle></CardHeader>
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

        <Card hover={false}>
          <CardHeader><CardTitle className="text-base">{t("dashboard.metadataCoverage")}</CardTitle></CardHeader>
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
      </div>

      {/* Metadata quality + provider */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        <Card hover={false}>
          <CardHeader><CardTitle className="text-base">{t("dashboard.byProvider")}</CardTitle></CardHeader>
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

        <Card hover={false}>
          <CardHeader><CardTitle className="text-base">{t("dashboard.bookMetadata")}</CardTitle></CardHeader>
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
    </>
  );
}
