import { Card, CardHeader, CardTitle, CardDescription, CardContent, ReportStatGrid } from "@/app/components/ui";
import type { DownloadDetectionReportDto, DownloadDetectionResultDto } from "@/lib/api";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";

interface RssRelease {
  title: string;
  indexer?: string | null;
  size: number;
  seeders?: number | null;
  leechers?: number | null;
  publish_date?: string | null;
  categories?: string[] | null;
}

function formatPublishDate(raw: string | null | undefined): string {
  if (!raw) return "—";
  const d = new Date(raw);
  if (isNaN(d.getTime())) return raw;
  return d.toLocaleDateString(undefined, { day: "2-digit", month: "short", hour: "2-digit", minute: "2-digit" });
}

export function RssSnapshotCard({ releases, t }: {
  releases: RssRelease[];
  t: TranslateFunction;
}) {
  return (
    <Card className="lg:col-span-2">
      <CardHeader>
        <CardTitle>{t("jobDetail.rssSnapshotTitle")}</CardTitle>
        <CardDescription>{t("jobDetail.rssSnapshotDesc", { count: String(releases.length) })}</CardDescription>
      </CardHeader>
      <CardContent>
        <div className="overflow-x-auto max-h-[500px] overflow-y-auto rounded-lg border border-border">
          <table className="w-full text-xs">
            <thead className="sticky top-0 bg-muted/80 backdrop-blur-sm z-10">
              <tr>
                <th className="text-left p-2 text-xs font-semibold text-muted-foreground uppercase tracking-wider">{t("jobDetail.rssColumnTitle")}</th>
                <th className="text-left p-2 text-xs font-semibold text-muted-foreground uppercase tracking-wider whitespace-nowrap">{t("jobDetail.rssColumnIndexer")}</th>
                <th className="text-left p-2 text-xs font-semibold text-muted-foreground uppercase tracking-wider whitespace-nowrap">{t("jobDetail.rssColumnCategories")}</th>
                <th className="text-right p-2 text-xs font-semibold text-muted-foreground uppercase tracking-wider whitespace-nowrap">{t("jobDetail.rssColumnSize")}</th>
                <th className="text-right p-2 text-xs font-semibold text-muted-foreground uppercase tracking-wider whitespace-nowrap">{t("jobDetail.rssColumnSeeders")}</th>
                <th className="text-right p-2 text-xs font-semibold text-muted-foreground uppercase tracking-wider whitespace-nowrap">{t("jobDetail.rssColumnDate")}</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-border/40">
              {releases.map((r, i) => (
                <tr key={i} className="hover:bg-accent/30 transition-colors">
                  <td className="p-2 font-mono text-foreground max-w-sm truncate" title={r.title}>{r.title}</td>
                  <td className="p-2 text-muted-foreground whitespace-nowrap">{r.indexer ?? "—"}</td>
                  <td className="p-2 text-muted-foreground">
                    {r.categories?.length ? (
                      <div className="flex flex-wrap gap-1">
                        {r.categories.map((c) => (
                          <span key={c} className="px-1.5 py-0.5 rounded bg-muted text-3xs whitespace-nowrap">{c}</span>
                        ))}
                      </div>
                    ) : "—"}
                  </td>
                  <td className="p-2 text-right text-muted-foreground whitespace-nowrap">
                    {r.size ? `${(r.size / 1024 / 1024).toFixed(0)} MB` : "—"}
                  </td>
                  <td className="p-2 text-right whitespace-nowrap">
                    {r.seeders != null ? (
                      <span className="text-success font-medium">{r.seeders}</span>
                    ) : "—"}
                  </td>
                  <td className="p-2 text-right text-muted-foreground whitespace-nowrap">
                    {formatPublishDate(r.publish_date)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </CardContent>
    </Card>
  );
}

export function RssPollStatsCard({
  stats,
  t,
}: {
  stats: { total_series?: number; found?: number; new_releases?: number; rss_releases_fetched?: number };
  t: TranslateFunction;
}) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("jobType.prowlarr_rssLabel")}</CardTitle>
        <CardDescription>
          {t("jobDetail.rssPollFetched", { count: String(stats.rss_releases_fetched ?? 0) })}
        </CardDescription>
      </CardHeader>
      <CardContent>
        <ReportStatGrid
          stats={[
            { value: stats.total_series ?? 0, label: t("jobDetail.seriesChecked") },
            { value: stats.found ?? 0, label: t("jobDetail.downloadFound"), variant: "success" },
            { value: stats.new_releases ?? 0, label: t("jobDetail.downloadNewReleases"), variant: "primary" },
          ]}
        />
      </CardContent>
    </Card>
  );
}

export function DownloadDetectionReportCard({ report, t }: { report: DownloadDetectionReportDto; t: TranslateFunction }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("jobDetail.downloadDetectionReport")}</CardTitle>
        <CardDescription>{t("jobDetail.seriesAnalyzed", { count: String(report.total_series) })}</CardDescription>
      </CardHeader>
      <CardContent>
        <ReportStatGrid
          stats={[
            { value: report.found, label: t("jobDetail.downloadFound"), variant: "success" },
            { value: report.new_releases ?? 0, label: t("jobDetail.downloadNewReleases"), variant: "primary" },
            { value: report.not_found, label: t("jobDetail.downloadNotFound") },
            { value: report.no_missing, label: t("jobDetail.downloadNoMissing"), variant: "primary" },
            { value: report.no_metadata, label: t("jobDetail.downloadNoMetadata") },
            { value: report.errors, label: t("jobDetail.errors"), variant: report.errors > 0 ? "error" : "default" },
          ]}
        />
      </CardContent>
    </Card>
  );
}

export function DownloadDetectionErrorsCard({ results, t }: {
  results: DownloadDetectionResultDto[];
  t: TranslateFunction;
}) {
  if (results.length === 0) return null;

  return (
    <Card className="lg:col-span-2">
      <CardHeader>
        <CardTitle>{t("jobDetail.downloadErrors")}</CardTitle>
        <CardDescription>{t("jobDetail.downloadErrorsDesc", { count: String(results.length) })}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-2 max-h-80 overflow-y-auto">
        {results.map((r) => (
          <div key={r.id} className="p-3 bg-destructive/10 rounded-lg border border-destructive/20">
            <p className="text-sm font-semibold text-destructive mb-1">{r.series_name}</p>
            <p className="text-sm text-destructive/80">{r.error_message ?? t("common.unknownError")}</p>
          </div>
        ))}
      </CardContent>
    </Card>
  );
}
