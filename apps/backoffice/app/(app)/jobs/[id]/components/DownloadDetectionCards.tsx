import Link from "next/link";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, StatBox } from "@/app/components/ui";
import { QbittorrentProvider, QbittorrentDownloadButton } from "@/app/components/QbittorrentDownloadButton";
import type { DownloadDetectionReportDto, DownloadDetectionResultDto } from "@/lib/api";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";

export function DownloadDetectionReportCard({ report, t }: { report: DownloadDetectionReportDto; t: TranslateFunction }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("jobDetail.downloadDetectionReport")}</CardTitle>
        <CardDescription>{t("jobDetail.seriesAnalyzed", { count: String(report.total_series) })}</CardDescription>
      </CardHeader>
      <CardContent>
        <div className="grid grid-cols-2 sm:grid-cols-3 gap-4">
          <StatBox value={report.found} label={t("jobDetail.downloadFound")} variant="success" />
          <StatBox value={report.not_found} label={t("jobDetail.downloadNotFound")} />
          <StatBox value={report.no_missing} label={t("jobDetail.downloadNoMissing")} variant="primary" />
          <StatBox value={report.no_metadata} label={t("jobDetail.downloadNoMetadata")} />
          <StatBox value={report.errors} label={t("jobDetail.errors")} variant={report.errors > 0 ? "error" : "default"} />
        </div>
      </CardContent>
    </Card>
  );
}

export function DownloadDetectionResultsCard({ results, libraryId, t }: {
  results: DownloadDetectionResultDto[];
  libraryId: string | null;
  t: TranslateFunction;
}) {
  if (results.length === 0) return null;

  return (
    <QbittorrentProvider>
      <Card className="lg:col-span-2">
        <CardHeader>
          <CardTitle>{t("jobDetail.downloadAvailableReleases")}</CardTitle>
          <CardDescription>{t("jobDetail.downloadAvailableReleasesDesc", { count: String(results.length) })}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4 max-h-[700px] overflow-y-auto">
          {results.map((r) => (
            <div key={r.id} className="rounded-lg border border-success/20 bg-success/5 p-3">
              <div className="flex items-center justify-between gap-2 mb-2">
                {libraryId ? (
                  <Link
                    href={`/libraries/${libraryId}/series/${encodeURIComponent(r.series_name)}`}
                    className="font-semibold text-sm text-primary hover:underline truncate"
                  >
                    {r.series_name}
                  </Link>
                ) : (
                  <span className="font-semibold text-sm text-foreground truncate">{r.series_name}</span>
                )}
                <span className="text-[10px] px-1.5 py-0.5 rounded-full font-medium whitespace-nowrap bg-warning/20 text-warning shrink-0">
                  {t("jobDetail.downloadMissingCount", { count: String(r.missing_count) })}
                </span>
              </div>
              {r.available_releases && r.available_releases.length > 0 && (
                <div className="space-y-1.5">
                  {r.available_releases.map((release, idx) => (
                    <div key={idx} className="flex items-center gap-2 p-2 rounded bg-background/60 border border-border/40">
                      <div className="flex-1 min-w-0">
                        <p className="text-xs font-mono text-foreground truncate" title={release.title}>{release.title}</p>
                        <div className="flex items-center gap-3 mt-1 flex-wrap">
                          {release.indexer && (
                            <span className="text-[10px] text-muted-foreground">{release.indexer}</span>
                          )}
                          {release.seeders != null && (
                            <span className="text-[10px] text-success font-medium">{release.seeders} {t("prowlarr.columnSeeders").toLowerCase()}</span>
                          )}
                          <span className="text-[10px] text-muted-foreground">
                            {(release.size / 1024 / 1024).toFixed(0)} MB
                          </span>
                          <div className="flex items-center gap-1">
                            {release.matched_missing_volumes.map((vol) => (
                              <span key={vol} className="text-[10px] px-1.5 py-0.5 rounded-full bg-success/20 text-success font-medium">
                                T.{vol}
                              </span>
                            ))}
                          </div>
                        </div>
                      </div>
                      {release.download_url && (
                        <QbittorrentDownloadButton downloadUrl={release.download_url} releaseId={`${r.id}-${idx}`} />
                      )}
                    </div>
                  ))}
                </div>
              )}
            </div>
          ))}
        </CardContent>
      </Card>
    </QbittorrentProvider>
  );
}
