import { fetchStatsOverview, fetchUsers, UserDto } from "@/lib/api";
import { Card, CardContent, CardHeader, CardTitle } from "@/app/components/ui";
import { RcDonutChart } from "@/app/components/DashboardCharts";
import { CurrentlyReadingList, RecentlyReadList } from "@/app/components/ReadingUserFilter";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";
import { formatBytes, formatNumber } from "@/lib/format";
import { StatCard } from "./StatCard";

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
